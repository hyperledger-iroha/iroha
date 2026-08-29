# Executed lexically in check_sumeragi_v2_proof_ledger.py; do not import directly.

def _production_trace_canonical_json_bytes(value: Any) -> bytes:
    """Encode the theorem certificate in its one accepted byte representation."""

    return (
        json.dumps(
            value,
            ensure_ascii=False,
            sort_keys=True,
            separators=(",", ":"),
        )
        + "\n"
    ).encode("utf-8")


def _production_trace_path_and_ancestor_snapshot(
    path: Path, *, label: str
) -> tuple[Path, tuple[tuple[Path, int, int], ...]]:
    """Resolve no links while pinning every directory leading to ``path``."""

    absolute = Path(os.path.abspath(path))
    if absolute.name in {"", ".", ".."}:
        raise ValueError(f"{label} path has no safe final component: {path}")
    parent = absolute.parent
    try:
        resolved_parent = parent.resolve(strict=True)
    except OSError as error:
        raise ValueError(f"{label} parent is unavailable: {parent}: {error}") from error
    if resolved_parent != parent:
        raise ValueError(
            f"{label} parent path contains a symlink component: {parent}"
        )
    snapshot: list[tuple[Path, int, int]] = []
    for ancestor in reversed((parent, *parent.parents)):
        try:
            metadata = ancestor.lstat()
        except OSError as error:
            raise ValueError(
                f"{label} ancestor is unavailable: {ancestor}: {error}"
            ) from error
        if stat.S_ISLNK(metadata.st_mode) or not stat.S_ISDIR(metadata.st_mode):
            raise ValueError(
                f"{label} ancestor must be a real directory: {ancestor}"
            )
        snapshot.append((ancestor, metadata.st_dev, metadata.st_ino))
    return absolute, tuple(snapshot)


def _production_trace_revalidate_ancestors(
    snapshot: tuple[tuple[Path, int, int], ...], *, label: str
) -> None:
    """Reject directory replacement or link insertion during evidence access."""

    for ancestor, expected_device, expected_inode in snapshot:
        try:
            metadata = ancestor.lstat()
        except OSError as error:
            raise ValueError(
                f"{label} ancestor disappeared: {ancestor}: {error}"
            ) from error
        if (
            stat.S_ISLNK(metadata.st_mode)
            or not stat.S_ISDIR(metadata.st_mode)
            or (metadata.st_dev, metadata.st_ino)
            != (expected_device, expected_inode)
        ):
            raise ValueError(f"{label} ancestor changed identity: {ancestor}")


def _bounded_regular_file_bytes(
    path: Path,
    *,
    label: str,
    maximum_bytes: int,
    allow_empty: bool = False,
) -> bytes:
    """Read one stable, singly linked file through a link-free path."""

    absolute, ancestors = _production_trace_path_and_ancestor_snapshot(
        path, label=label
    )
    try:
        named_before = absolute.lstat()
    except OSError as error:
        raise ValueError(
            f"{label} is not an available non-symlink file: {path}: {error}"
        ) from error
    if stat.S_ISLNK(named_before.st_mode) or not stat.S_ISREG(named_before.st_mode):
        raise ValueError(f"{label} is not a regular non-symlink file: {path}")
    if named_before.st_nlink != 1:
        raise ValueError(f"{label} must have exactly one hard link: {path}")
    flags = (
        os.O_RDONLY
        | getattr(os, "O_CLOEXEC", 0)
        | getattr(os, "O_NOFOLLOW", 0)
    )
    try:
        descriptor = os.open(absolute, flags)
    except OSError as error:
        raise ValueError(
            f"{label} is not an available non-symlink file: {path}: {error}"
        ) from error
    try:
        before = os.fstat(descriptor)
        if (
            not stat.S_ISREG(before.st_mode)
            or (before.st_dev, before.st_ino)
            != (named_before.st_dev, named_before.st_ino)
            or before.st_mode != named_before.st_mode
            or before.st_uid != named_before.st_uid
            or before.st_nlink != 1
        ):
            raise ValueError(f"{label} changed while it was opened: {path}")
        if (
            (not allow_empty and before.st_size == 0)
            or before.st_size > maximum_bytes
        ):
            qualifier = "non-empty and " if not allow_empty else ""
            raise ValueError(
                f"{label} must be {qualifier}at most {maximum_bytes} bytes: "
                f"{path} has {before.st_size} bytes"
            )
        chunks: list[bytes] = []
        remaining = maximum_bytes + 1
        while remaining > 0:
            chunk = os.read(descriptor, min(1024 * 1024, remaining))
            if not chunk:
                break
            chunks.append(chunk)
            remaining -= len(chunk)
        payload = b"".join(chunks)
        if len(payload) > maximum_bytes:
            raise ValueError(f"{label} exceeds {maximum_bytes} bytes: {path}")
        after = os.fstat(descriptor)
        stable_fields = (
            "st_dev",
            "st_ino",
            "st_size",
            "st_mtime_ns",
            "st_ctime_ns",
            "st_mode",
            "st_uid",
            "st_nlink",
        )
        if any(
            getattr(before, field) != getattr(after, field)
            for field in stable_fields
        ):
            raise ValueError(f"{label} changed while it was being read: {path}")
    finally:
        os.close(descriptor)
    try:
        named = absolute.lstat()
    except OSError as error:
        raise ValueError(
            f"{label} disappeared after it was read: {path}: {error}"
        ) from error
    if (
        stat.S_ISLNK(named.st_mode)
        or not stat.S_ISREG(named.st_mode)
        or named.st_nlink != 1
    ):
        raise ValueError(
            f"{label} path is no longer a regular non-symlink file: {path}"
        )
    named_fields = (
        "st_dev",
        "st_ino",
        "st_size",
        "st_mtime_ns",
        "st_ctime_ns",
        "st_mode",
        "st_uid",
        "st_nlink",
    )
    if any(getattr(named, field) != getattr(after, field) for field in named_fields):
        raise ValueError(
            f"{label} path changed identity while it was being read: {path}"
        )
    _production_trace_revalidate_ancestors(ancestors, label=label)
    return payload


def load_production_trace_extraction_evidence(path: Path) -> dict[str, Any]:
    """Load one bounded certificate and reject every non-canonical encoding."""

    payload = _bounded_regular_file_bytes(
        path,
        label="production trace-extraction evidence",
        maximum_bytes=PRODUCTION_TRACE_EXTRACTION_EVIDENCE_MAX_BYTES,
    )
    try:
        source = payload.decode("utf-8")
    except UnicodeDecodeError as error:
        raise ValueError(
            "production trace-extraction evidence is not UTF-8"
        ) from error
    try:
        document = json.loads(source, object_pairs_hook=_unique_object)
    except (json.JSONDecodeError, DuplicateKeyError) as error:
        raise ValueError(
            f"production trace-extraction evidence is invalid JSON: {error}"
        ) from error
    if not isinstance(document, dict):
        raise ValueError(
            "production trace-extraction evidence must be a JSON object"
        )
    if payload != _production_trace_canonical_json_bytes(document):
        raise ValueError(
            "production trace-extraction evidence is not canonical compact "
            "sorted-key JSON with one LF"
        )
    return document


def write_production_trace_extraction_evidence(
    path: Path, document: dict[str, Any]
) -> None:
    """Atomically publish one bounded canonical theorem certificate."""

    payload = _production_trace_canonical_json_bytes(document)
    if not payload or len(payload) > PRODUCTION_TRACE_EXTRACTION_EVIDENCE_MAX_BYTES:
        raise ValueError(
            "production trace-extraction evidence exceeds its canonical "
            f"{PRODUCTION_TRACE_EXTRACTION_EVIDENCE_MAX_BYTES}-byte bound"
        )
    absolute, ancestors = _production_trace_path_and_ancestor_snapshot(
        path, label="production trace-extraction evidence output"
    )
    directory_flags = (
        os.O_RDONLY
        | getattr(os, "O_DIRECTORY", 0)
        | getattr(os, "O_CLOEXEC", 0)
        | getattr(os, "O_NOFOLLOW", 0)
    )
    try:
        directory_descriptor = os.open(absolute.parent, directory_flags)
    except OSError as error:
        raise ValueError(
            "production trace-extraction evidence parent could not be opened safely"
        ) from error
    temporary_name = f".{absolute.name}.{secrets.token_hex(16)}.partial"
    temporary_identity: tuple[int, int] | None = None
    try:
        opened_parent = os.fstat(directory_descriptor)
        _, parent_device, parent_inode = ancestors[-1]
        if (
            not stat.S_ISDIR(opened_parent.st_mode)
            or (opened_parent.st_dev, opened_parent.st_ino)
            != (parent_device, parent_inode)
            or opened_parent.st_uid != os.geteuid()
            or stat.S_IMODE(opened_parent.st_mode) & 0o022
        ):
            raise ValueError(
                "production trace-extraction evidence parent must remain an "
                "owner-owned, non-group-writable real directory"
            )
        try:
            existing = os.stat(
                absolute.name,
                dir_fd=directory_descriptor,
                follow_symlinks=False,
            )
        except FileNotFoundError:
            existing = None
        if existing is not None:
            if stat.S_ISLNK(existing.st_mode) or not stat.S_ISREG(existing.st_mode):
                raise ValueError(
                    "refusing to replace a non-regular or symlinked production "
                    "trace certificate"
                )
            if existing.st_nlink != 1:
                raise ValueError(
                    "production trace-extraction evidence output must have "
                    "exactly one hard link"
                )
        create_flags = (
            os.O_RDWR
            | os.O_CREAT
            | os.O_EXCL
            | getattr(os, "O_CLOEXEC", 0)
            | getattr(os, "O_NOFOLLOW", 0)
        )
        descriptor = os.open(
            temporary_name,
            create_flags,
            0o600,
            dir_fd=directory_descriptor,
        )
        try:
            opened = os.fstat(descriptor)
            temporary_identity = (opened.st_dev, opened.st_ino)
            if not stat.S_ISREG(opened.st_mode) or opened.st_nlink != 1:
                raise ValueError(
                    "production trace-extraction evidence stage is not one "
                    "regular file"
                )
            written_bytes = 0
            while written_bytes < len(payload):
                count = os.write(descriptor, payload[written_bytes:])
                if count <= 0:
                    raise ValueError(
                        "production trace-extraction evidence stage write stalled"
                    )
                written_bytes += count
            os.fsync(descriptor)
            written = os.fstat(descriptor)
            if (
                (written.st_dev, written.st_ino) != temporary_identity
                or written.st_nlink != 1
                or written.st_size != len(payload)
            ):
                raise ValueError(
                    "production trace-extraction evidence stage metadata changed"
                )
            os.lseek(descriptor, 0, os.SEEK_SET)
            readback = bytearray()
            while len(readback) < len(payload):
                chunk = os.read(
                    descriptor, min(1024 * 1024, len(payload) - len(readback))
                )
                if not chunk:
                    break
                readback.extend(chunk)
            if bytes(readback) != payload:
                raise ValueError(
                    "production trace-extraction evidence stage failed byte "
                    "verification"
                )
        finally:
            os.close(descriptor)
        try:
            current = os.stat(
                absolute.name,
                dir_fd=directory_descriptor,
                follow_symlinks=False,
            )
        except FileNotFoundError:
            current = None
        if existing is None and current is not None:
            raise ValueError(
                "production trace-extraction evidence output appeared before "
                "publication"
            )
        if existing is not None and (
            current is None
            or (current.st_dev, current.st_ino, current.st_nlink)
            != (existing.st_dev, existing.st_ino, 1)
        ):
            raise ValueError(
                "production trace-extraction evidence output changed before publication"
            )
        _production_trace_revalidate_ancestors(
            ancestors, label="production trace-extraction evidence output"
        )
        staged = os.stat(
            temporary_name,
            dir_fd=directory_descriptor,
            follow_symlinks=False,
        )
        if (
            temporary_identity is None
            or not stat.S_ISREG(staged.st_mode)
            or (staged.st_dev, staged.st_ino) != temporary_identity
            or staged.st_nlink != 1
            or staged.st_size != len(payload)
        ):
            raise ValueError(
                "production trace-extraction evidence stage changed before "
                "publication"
            )
        os.replace(
            temporary_name,
            absolute.name,
            src_dir_fd=directory_descriptor,
            dst_dir_fd=directory_descriptor,
        )
        os.fsync(directory_descriptor)
        published = os.stat(
            absolute.name,
            dir_fd=directory_descriptor,
            follow_symlinks=False,
        )
        if (
            temporary_identity is None
            or not stat.S_ISREG(published.st_mode)
            or (published.st_dev, published.st_ino) != temporary_identity
            or published.st_nlink != 1
            or published.st_size != len(payload)
        ):
            raise ValueError(
                "production trace-extraction evidence publication identity is invalid"
            )
        _production_trace_revalidate_ancestors(
            ancestors, label="production trace-extraction evidence output"
        )
    finally:
        try:
            staged = os.stat(
                temporary_name,
                dir_fd=directory_descriptor,
                follow_symlinks=False,
            )
        except FileNotFoundError:
            staged = None
        if (
            staged is not None
            and temporary_identity is not None
            and (staged.st_dev, staged.st_ino) == temporary_identity
        ):
            os.unlink(temporary_name, dir_fd=directory_descriptor)
        os.close(directory_descriptor)


def _sha256_bytes(payload: bytes) -> str:
    return hashlib.sha256(payload).hexdigest()


def _production_trace_artifact_entry(role: str, path: Path) -> dict[str, Any]:
    payload = _bounded_regular_file_bytes(
        path,
        label=f"production trace component {role}",
        maximum_bytes=PRODUCTION_TRACE_EXTRACTION_COMPONENT_MAX_BYTES,
    )
    return {
        "role": role,
        "sha256": _sha256_bytes(payload),
        "size_bytes": len(payload),
    }


def _production_trace_rust_item_entry(
    *, path: str, kind: str, symbol: str, item: RustItem
) -> dict[str, Any]:
    return {
        "path": path,
        "kind": kind,
        "symbol": symbol,
        "source_sha256": _sha256_bytes(item.source.encode("utf-8")),
        "token_sha256": _rust_sealed_item_token_sha256(item),
    }


def _production_trace_tla_symbol_entry(
    path: str, symbol: str, body: str
) -> dict[str, Any]:
    tokens = (symbol, *tla_code_tokens(body))
    return {
        "path": path,
        "kind": "tla_operator",
        "symbol": symbol,
        "token_sha256": _sha256_bytes("\0".join(tokens).encode("utf-8")),
    }


def _load_multilane_model_checker() -> Any:
    path = Path(__file__).with_name("check_sumeragi_v2_multilane_models.py")
    if path.is_symlink() or not path.is_file():
        raise ValueError(f"multilane source checker is unavailable: {path}")
    spec = importlib.util.spec_from_file_location(
        "_sumeragi_v2_multilane_models_for_trace_certificate", path
    )
    if spec is None or spec.loader is None:
        raise ValueError(f"cannot load multilane source checker: {path}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def _production_trace_unique_function(
    *,
    root_dir: Path,
    relative: str,
    symbol: str,
    impl_name: str | None,
    errors: list[str],
) -> RustItem | None:
    path = root_dir / relative
    try:
        payload = _bounded_regular_file_bytes(
            path,
            label=f"production trace source {relative}",
            maximum_bytes=PRODUCTION_TRACE_EXTRACTION_COMPONENT_MAX_BYTES,
        )
        source = payload.decode("utf-8")
    except (UnicodeDecodeError, ValueError) as error:
        errors.append(str(error))
        return None
    items = rust_items(source, symbol)
    expected_context = (
        None
        if impl_name is None
        else (tuple(rust_code_tokens(f"impl {impl_name}")),)
    )
    matches = [
        item
        for item in items
        if expected_context is None or item.brace_context == expected_context
    ]
    qualified = symbol if impl_name is None else f"{impl_name}::{symbol}"
    if len(matches) != 1:
        errors.append(
            f"production trace-extraction theorem requires exactly one non-macro "
            f"item {relative}!{qualified}; found {len(matches)}"
        )
        return None
    item = matches[0]
    if _rust_item_is_test_only(item):
        errors.append(
            f"production trace-extraction theorem item {relative}!{qualified} "
            "is test-only"
        )
        return None
    gated = [
        attribute
        for attribute in (*item.attributes, *item.ancestor_inner_attributes)
        if re.search(r"(?s)^#\s*!?\s*\[\s*cfg(?:_attr)?\b", attribute)
    ]
    if gated:
        errors.append(
            f"production trace-extraction theorem item {relative}!{qualified} "
            f"is configuration-gated: {gated!r}"
        )
        return None
    return item


def _production_trace_extraction_source_snapshot(
    *, root_dir: Path = ROOT_DIR, formal_dir: Path = FORMAL_DIR
) -> dict[str, Any]:
    """Authenticate and hash the exact cross-language trace-extraction seam."""

    errors: list[str] = []
    root_dir = root_dir.resolve()
    model_relative = "formal/sumeragi_v2/SumeragiV2InFlightFirstRelease.tla"
    bindings_relative = "formal/sumeragi_v2/multilane_source_bindings.json"
    model_path = root_dir / model_relative
    try:
        model_payload = _bounded_regular_file_bytes(
            model_path,
            label="in-flight first-release production model",
            maximum_bytes=PRODUCTION_TRACE_EXTRACTION_COMPONENT_MAX_BYTES,
        )
        model_source = model_payload.decode("utf-8")
    except (UnicodeDecodeError, ValueError) as error:
        errors.append(str(error))
        model_payload = b""
        model_source = ""

    try:
        binding_ledger = json.loads(
            _bounded_regular_file_bytes(
                root_dir / bindings_relative,
                label="multilane source-binding ledger",
                maximum_bytes=PRODUCTION_TRACE_EXTRACTION_COMPONENT_MAX_BYTES,
            ).decode("utf-8")
        )
    except (OSError, UnicodeDecodeError, ValueError) as error:
        errors.append(
            "production trace-extraction theorem cannot read its model-action "
            f"inventory: {error}"
        )
    else:
        layout_contract = (
            binding_ledger.get("inflight_first_release_layout_contract")
            if isinstance(binding_ledger, dict)
            else None
        )
        ledger_actions = (
            layout_contract.get("required_actions")
            if isinstance(layout_contract, dict)
            else None
        )
        if not isinstance(ledger_actions, list) or tuple(ledger_actions) != (
            PRODUCTION_TRACE_EXTRACTION_REQUIRED_MODEL_ACTIONS
        ):
            errors.append(
                "production trace-extraction required model actions differ from "
                "the multilane source-binding ledger"
            )

    errors.extend(_production_trace_extraction_action_partition_errors())
    ordered_actions: list[str] = []
    for binding in PRODUCTION_TRACE_EXTRACTION_BINDINGS:
        for action in binding["model_actions"]:
            if action not in ordered_actions:
                ordered_actions.append(action)
    for open_model_symbol in (
        "RecoverReservationSnapshot",
        "RehydrateLocalKuraCustody",
    ):
        if open_model_symbol not in ordered_actions:
            ordered_actions.append(open_model_symbol)
    model_symbols: list[dict[str, Any]] = []
    for symbol in (
        *ordered_actions,
        "Next",
        "ConflictingPayloadBindingMutation",
        "MLExecutionInputBeforeReadyAuthorization",
        "MLLaneCommitBeforeAtomicWsvCarrierApplication",
        "MLExactlyOnceCarrierApplication",
        "MLPostCarrierCommitCleanupOrder",
    ):
        extracted = _top_level_operator_body(model_source, symbol)
        if extracted is None:
            errors.append(
                f"production trace-extraction theorem model lacks operator {symbol}"
            )
            continue
        model_symbols.append(
            _production_trace_tla_symbol_entry(model_relative, symbol, extracted[0])
        )
        if symbol == "ConflictingPayloadBindingMutation":
            mutation_tokens = tla_code_tokens(extracted[0])
            for required in (
                'Mode = "PayloadBindingConflict"',
                "payloadBinding'",
                "BindingB",
            ):
                if _token_sequence_count(
                    mutation_tokens,
                    tla_code_tokens(required),
                ) != 1:
                    errors.append(
                        "production operational-correspondence requires the "
                        "unmapped payload-binding mutation to remain explicitly "
                        f"test-mode-only: missing {required!r}"
                    )

    core_relative = "crates/iroha_core/src/sumeragi/v2_core/refinement.rs"
    post_carrier_relative = (
        "crates/iroha_core/src/sumeragi/v2_core/refinement/"
        "post_carrier_transition.rs"
    )
    core_items: list[dict[str, Any]] = []
    for relative, symbol in (
        (
            post_carrier_relative,
            "check_production_in_flight_first_release_transition",
        ),
        (
            post_carrier_relative,
            "check_production_in_flight_first_release_rehydrate_local_kura_custody_transition",
        ),
        (core_relative, "production_in_flight_first_release_transition_kernel"),
        (core_relative, "production_in_flight_first_release_witness_binding_kernel"),
        (post_carrier_relative, "production_in_flight_first_release_terminal_owner"),
    ):
        item = _production_trace_unique_function(
            root_dir=root_dir,
            relative=relative,
            symbol=symbol,
            impl_name=None,
            errors=errors,
        )
        if item is not None:
            core_items.append(
                _production_trace_rust_item_entry(
                    path=relative, kind="fn", symbol=symbol, item=item
                )
            )
    _core_path, core_source = _read_reviewed_rust_source(
        root_dir,
        core_relative,
        errors,
        "production first-release refinement kernel",
    )
    if len(core_source.encode("utf-8")) > PRODUCTION_TRACE_EXTRACTION_COMPONENT_MAX_BYTES:
        errors.append(
            "production first-release refinement kernel exceeds the bounded "
            "trace-extraction component limit"
        )
        core_source = ""
    transition_macros = rust_macro_items(
        core_source, "production_in_flight_first_release_transition_body"
    )
    if len(transition_macros) != 1:
        errors.append(
            "production trace-extraction theorem requires exactly one "
            "production_in_flight_first_release_transition_body macro; "
            f"found {len(transition_macros)}"
        )
    else:
        core_items.append(
            _production_trace_rust_item_entry(
                path=core_relative,
                kind="macro",
                symbol="production_in_flight_first_release_transition_body",
                item=transition_macros[0],
            )
        )

    witness_structs = rust_struct_items(
        core_source,
        "ProductionInFlightFirstReleaseTransitionWitnessV1",
    )
    if len(witness_structs) != 1 or _rust_item_is_test_only(witness_structs[0]):
        errors.append(
            "production trace-extraction theorem requires exactly one "
            "non-test ProductionInFlightFirstReleaseTransitionWitnessV1 struct"
        )
    else:
        core_items.append(
            _production_trace_rust_item_entry(
                path=core_relative,
                kind="struct",
                symbol="ProductionInFlightFirstReleaseTransitionWitnessV1",
                item=witness_structs[0],
            )
        )
    witness_binding_macros = rust_macro_items(
        core_source,
        "production_in_flight_first_release_witness_binding_body",
    )
    if len(witness_binding_macros) != 1:
        errors.append(
            "production trace-extraction theorem requires exactly one "
            "production_in_flight_first_release_witness_binding_body macro"
        )
    else:
        core_items.append(
            _production_trace_rust_item_entry(
                path=core_relative,
                kind="macro",
                symbol="production_in_flight_first_release_witness_binding_body",
                item=witness_binding_macros[0],
            )
        )

    action_mappings = PRODUCTION_TRACE_EXTRACTION_ACTION_WITNESS_MAPPINGS
    mapped_model_actions = tuple(mapping[0] for mapping in action_mappings)
    mapped_action_tags = tuple(mapping[1] for mapping in action_mappings)
    mapped_discriminants = tuple(mapping[2] for mapping in action_mappings)
    if mapped_model_actions != PRODUCTION_TRACE_EXTRACTION_REQUIRED_MODEL_ACTIONS:
        errors.append(
            "production operational-correspondence mapping has an unmapped, "
            "unexpected, or reordered model action"
        )
    if len(set(mapped_model_actions)) != len(mapped_model_actions):
        errors.append(
            "production operational-correspondence mapping contains a duplicate model action"
        )
    if len(set(mapped_action_tags)) != len(mapped_action_tags):
        errors.append(
            "production operational-correspondence mapping contains a duplicate Rust action tag"
        )
    if set(mapped_discriminants) != set(range(1, 28)) or len(
        set(mapped_discriminants)
    ) != len(mapped_discriminants):
        errors.append(
            "production operational-correspondence mapping must use each V1 "
            "discriminant from 1 through 27 exactly once"
        )

    core_statements = rust_top_level_statements(core_source)
    action_mapping_entries: list[dict[str, Any]] = []
    transition_macro_tokens = (
        () if len(transition_macros) != 1 else rust_code_tokens(transition_macros[0].source)
    )
    for model_action, action_tag, discriminant in action_mappings:
        expected_statement = rust_code_tokens(
            f"pub(crate) const {action_tag}: u8 = {discriminant};"
        )
        matching_statements = [
            statement
            for statement in core_statements
            if statement.tokens == expected_statement
        ]
        kernel_occurrences = _token_sequence_count(
            transition_macro_tokens,
            rust_code_tokens(f"refinement_tag_value!({action_tag})"),
        )
        if len(matching_statements) != 1:
            errors.append(
                "production operational-correspondence action tag definition "
                f"is missing or ambiguous for {model_action}: {action_tag}={discriminant}"
            )
            continue
        if kernel_occurrences != 1:
            errors.append(
                "production operational-correspondence action must have exactly "
                f"one shared-kernel arm for {model_action}; found {kernel_occurrences}"
            )
            continue
        statement = matching_statements[0]
        action_mapping_entries.append(
            {
                "model_action": model_action,
                "rust_action_tag": action_tag,
                "discriminant": discriminant,
                "tag_source_sha256": _sha256_bytes(
                    statement.source.encode("utf-8")
                ),
                "tag_token_sha256": _rust_statement_token_sha256(statement),
                "shared_kernel_occurrences": kernel_occurrences,
            }
        )

    verus_relative = (
        "crates/iroha_sumeragi_core/src/verus_proofs/"
        "in_flight_first_release_proofs.rs"
    )
    verus_items: list[dict[str, Any]] = []
    for symbol in (
        "production_in_flight_first_release_transition_refines_named_next",
        "production_in_flight_first_release_witness_refines_named_next",
        "production_in_flight_reservation_snapshot_replay_refines_composed_stutter",
        "production_in_flight_first_release_snapshot_recovery_is_stutter",
        "production_in_flight_first_release_local_kura_rehydration_is_exact",
        "production_in_flight_first_release_local_kura_rehydration_rejects_missing_payload",
        "production_in_flight_first_release_local_kura_rehydration_rejects_volatile_drift",
        "production_in_flight_first_release_local_kura_rehydration_rejects_terminal_state",
        "production_in_flight_first_release_terminal_owner_is_exclusive",
    ):
        item = _production_trace_unique_function(
            root_dir=root_dir,
            relative=verus_relative,
            symbol=symbol,
            impl_name=None,
            errors=errors,
        )
        if item is None:
            continue
        verus_items.append(
            _production_trace_rust_item_entry(
                path=verus_relative, kind="verus_proof_fn", symbol=symbol, item=item
            )
        )
    if verus_items:
        first_theorem = _production_trace_unique_function(
            root_dir=root_dir,
            relative=verus_relative,
            symbol="production_in_flight_first_release_transition_refines_named_next",
            impl_name=None,
            errors=[],
        )
        if first_theorem is not None:
            required = rust_code_tokens(
                "check_production_in_flight_first_release_transition(projection) "
                "== Some(projection) ==> "
                "production_in_flight_first_release_transition_kernel(projection)"
            )
            if (
                _token_sequence_count(
                    rust_code_tokens(first_theorem.source), required
                )
                != 1
            ):
                errors.append(
                    "Verus production_in_flight_first_release_transition_"
                    "refines_named_next "
                    "does not retain its exact checked-transition implication"
                )

    witness_theorem = _production_trace_unique_function(
        root_dir=root_dir,
        relative=verus_relative,
        symbol="production_in_flight_first_release_witness_refines_named_next",
        impl_name=None,
        errors=[],
    )
    if witness_theorem is not None:
        witness_theorem_tokens = rust_code_tokens(witness_theorem.source)
        for required in (
            "production_in_flight_first_release_transition_kernel(projection)",
            "production_in_flight_first_release_witness_binding_kernel(projection, witness)",
            "witness.action == projection.action",
            "witness.actor == projection.actor",
            "witness.target == projection.target",
        ):
            if _token_sequence_count(
                witness_theorem_tokens,
                rust_code_tokens(required),
            ) == 0:
                errors.append(
                    "Verus first-release witness theorem lost required structural "
                    f"binding {required!r}"
                )

    verus_kernel_relative = verus_relative
    verus_witness_kernel = _production_trace_unique_function(
        root_dir=root_dir,
        relative=verus_kernel_relative,
        symbol="production_in_flight_first_release_witness_binding_kernel",
        impl_name=None,
        errors=errors,
    )
    if verus_witness_kernel is not None:
        verus_items.append(
            _production_trace_rust_item_entry(
                path=verus_kernel_relative,
                kind="verus_spec_fn",
                symbol="production_in_flight_first_release_witness_binding_kernel",
                item=verus_witness_kernel,
            )
        )

    shared_identity_relative = "crates/iroha_core/src/queue.rs"
    shared_identity_symbol = (
        "canonical_lane_queue_reservation_group_identity_projection"
    )
    shared_identity_item = _production_trace_unique_function(
        root_dir=root_dir,
        relative=shared_identity_relative,
        symbol=shared_identity_symbol,
        impl_name=None,
        errors=errors,
    )
    shared_identity_entry = None
    if shared_identity_item is not None:
        shared_identity_tokens = rust_code_tokens(shared_identity_item.source)
        for required_token in (
            "reservation_group_hash",
            "IDENTITY_DOMAIN_PAYLOAD",
            "IDENTITY_KIND_CANONICAL_PAYLOAD",
        ):
            if _token_sequence_count(
                shared_identity_tokens, rust_code_tokens(required_token)
            ) != 1:
                errors.append(
                    "production trace-extraction shared carrier identity must "
                    f"contain exactly one {required_token} token"
                )
        shared_identity_entry = _production_trace_rust_item_entry(
            path=shared_identity_relative,
            kind="fn",
            symbol=shared_identity_symbol,
            item=shared_identity_item,
        )

    production_items: list[dict[str, Any]] = []
    operational_relative = "crates/iroha_core/src/sumeragi/v2_core.rs"
    operational_items: list[dict[str, Any]] = []
    operational_source = ""
    try:
        operational_source = _bounded_regular_file_bytes(
            root_dir / operational_relative,
            label="production first-release operational-correspondence wrapper",
            maximum_bytes=PRODUCTION_TRACE_EXTRACTION_COMPONENT_MAX_BYTES,
        ).decode("utf-8")
    except (UnicodeDecodeError, ValueError) as error:
        errors.append(str(error))

    operational_required_tokens = {
        "canonical_first_release_state_bytes_v1": (
            "state.validator_count",
            "state.producer",
            "state.producer_selected_owner",
            "state.replicated_carrier_owners",
            "state.payload_binding_a",
            "state.binding_a",
            "state.queue.plan_state",
            "state.queue.selected_count",
            "state.queue.reservation_state",
            "state.carrier.kura_active",
            "state.carrier.execution_input_durable",
            "state.carrier.ready_qc_durable",
            "state.session.bodies",
            "state.session.ready_authorized",
            "state.session.crashed",
            "state.session.producer_alive",
            "state.history.ever_queue_plan_v1",
            "state.history.ever_reservation_v1",
            "state.history.ever_execution_input_durable",
            "state.history.ever_ready_authorized",
            "state.history.ready_signed",
            "state.history.ever_ready_qc_durable",
            "state.history.reservation_committed_prefix",
            "state.history.queue_plan_tombstoned_prefix",
            "state.history.reservation_commit_forgotten_prefix",
            "state.history.pending_high_water",
            "state.history.released_high_water",
            "state.decision.lane_commit_scope",
            "state.decision.release_scope",
            "state.decision.lane_commit_owner",
            "state.decision.release_owner",
            "state.decision.wsv_committed",
            "state.decision.application_count",
            "state.decision.applied_by",
            "state.release.kura_retired",
            "state.release.pending_prefix",
            "state.release.released_prefix",
            "state.release.fifo_restored",
        ),
        "production_in_flight_first_release_state_digest_v1": (
            "iroha_crypto::sha256(canonical_first_release_state_bytes_v1(state))",
        ),
        "production_in_flight_first_release_transition_witness_v1": (
            "schema_version: PRODUCTION_IN_FLIGHT_FIRST_RELEASE_TRANSITION_WITNESS_VERSION",
            "action: projection.action",
            "actor: projection.actor",
            "target: projection.target",
            "before_state_digest: production_in_flight_first_release_state_digest_v1(projection.before)",
            "after_state_digest: production_in_flight_first_release_state_digest_v1(projection.after)",
            "source_identity: PRODUCTION_IN_FLIGHT_FIRST_RELEASE_TLA_SOURCE_SHA256",
        ),
        "authenticate_production_in_flight_first_release_transition_witness_v1": (
            "refinement::production_in_flight_first_release_transition_kernel(projection)",
            "production_in_flight_first_release_witness_binding_kernel(projection, witness)",
            "witness == production_in_flight_first_release_transition_witness_v1(projection)",
        ),
        "check_production_in_flight_first_release_replay_step_v1": (
            "ProductionInFlightFirstReleaseReplayStepV1::ComposedNext",
            "projection.action != IN_FLIGHT_FIRST_RELEASE_ACTION_RECOVER_RESERVATION_SNAPSHOT",
            "projection.action != IN_FLIGHT_FIRST_RELEASE_ACTION_REPAIR_POST_CARRIER",
            "projection.before != projection.after",
            "ProductionInFlightFirstReleaseReplayStepV1::ReleaseReservationDirectProofStutter",
            "projection.action == IN_FLIGHT_FIRST_RELEASE_ACTION_RELEASE_RESERVATION_DIRECT",
            "ProductionInFlightFirstReleaseReplayStepV1::"
            "ReleaseReservationDirectProofStutter => { "
            "projection.action == "
            "IN_FLIGHT_FIRST_RELEASE_ACTION_RELEASE_RESERVATION_DIRECT "
            "&& projection.before == projection.after }",
            "ProductionInFlightFirstReleaseReplayStepV1::RecoverReservationSnapshotStutter",
            "projection.before == projection.after",
            "ProductionInFlightFirstReleaseReplayStepV1::RepairPostCarrierEvidenceStutter",
            "refinement::check_production_in_flight_first_release_transition(projection)",
            "authenticate_production_in_flight_first_release_transition_witness_v1(projection, witness)",
            "checked.with_first_release_witness(witness)",
        ),
        "check_production_in_flight_first_release_transition": (
            "IN_FLIGHT_FIRST_RELEASE_ACTION_RELEASE_RESERVATION_DIRECT",
            "ProductionInFlightFirstReleaseReplayStepV1::ReleaseReservationDirectProofStutter",
            "IN_FLIGHT_FIRST_RELEASE_ACTION_RELEASE_RESERVATION_DIRECT "
            "if projection.before == projection.after => { "
            "ProductionInFlightFirstReleaseReplayStepV1::"
            "ReleaseReservationDirectProofStutter }",
            "IN_FLIGHT_FIRST_RELEASE_ACTION_RECOVER_RESERVATION_SNAPSHOT",
            "ProductionInFlightFirstReleaseReplayStepV1::RecoverReservationSnapshotStutter",
            "IN_FLIGHT_FIRST_RELEASE_ACTION_REPAIR_POST_CARRIER",
            "ProductionInFlightFirstReleaseReplayStepV1::RepairPostCarrierEvidenceStutter",
            "_ => ProductionInFlightFirstReleaseReplayStepV1::ComposedNext",
            "check_production_in_flight_first_release_replay_step_v1(projection, classification)",
        ),
    }
    for symbol, required_tokens in operational_required_tokens.items():
        item = _production_trace_unique_function(
            root_dir=root_dir,
            relative=operational_relative,
            symbol=symbol,
            impl_name=None,
            errors=errors,
        )
        if item is None:
            continue
        item_tokens = rust_code_tokens(item.source)
        missing = [
            token
            for token in required_tokens
            if _token_sequence_count(item_tokens, rust_code_tokens(token)) == 0
        ]
        if missing:
            errors.append(
                "production operational-correspondence wrapper is incomplete at "
                f"{symbol}: missing exact code tokens {missing!r}"
            )
            continue
        entry = _production_trace_rust_item_entry(
            path=operational_relative,
            kind="fn",
            symbol=symbol,
            item=item,
        )
        operational_items.append(entry)
        production_items.append(entry)

    replay_enums = rust_enum_items(
        operational_source,
        "ProductionInFlightFirstReleaseReplayStepV1",
    )
    if len(replay_enums) != 1 or _rust_item_is_test_only(replay_enums[0]):
        errors.append(
            "production operational-correspondence requires exactly one non-test "
            "ProductionInFlightFirstReleaseReplayStepV1 enum"
        )
    else:
        replay_enum_tokens = rust_code_tokens(replay_enums[0].source)
        for variant in (
            "ComposedNext",
            "ReleaseReservationDirectProofStutter",
            "RecoverReservationSnapshotStutter",
            "RepairPostCarrierEvidenceStutter",
        ):
            if _token_sequence_count(replay_enum_tokens, rust_code_tokens(variant)) != 1:
                errors.append(
                    "production trace replay classification must define exactly "
                    f"one {variant} variant"
                )
        entry = _production_trace_rust_item_entry(
            path=operational_relative,
            kind="enum",
            symbol="ProductionInFlightFirstReleaseReplayStepV1",
            item=replay_enums[0],
        )
        operational_items.append(entry)
        production_items.append(entry)

    operational_statements = rust_top_level_statements(operational_source)
    model_source_identity = _sha256_bytes(model_payload)
    identity_words = [
        model_source_identity[offset : offset + 16]
        for offset in range(0, 64, 16)
    ]
    identity_literals = [
        "0x" + "_".join(word[index : index + 4] for index in range(0, 16, 4))
        for word in identity_words
    ]
    expected_operational_statements = {
        "PRODUCTION_IN_FLIGHT_FIRST_RELEASE_TRANSITION_WITNESS_VERSION": rust_code_tokens(
            "pub(crate) const PRODUCTION_IN_FLIGHT_FIRST_RELEASE_TRANSITION_WITNESS_VERSION: u16 = 1;"
        ),
        "PRODUCTION_IN_FLIGHT_FIRST_RELEASE_TLA_SOURCE_SHA256": rust_code_tokens(
            "pub(crate) const PRODUCTION_IN_FLIGHT_FIRST_RELEASE_TLA_SOURCE_SHA256: "
            "ProductionDigest256Projection = ProductionDigest256Projection { "
            f"word0: {identity_literals[0]}, word1: {identity_literals[1]}, "
            f"word2: {identity_literals[2]}, word3: {identity_literals[3]}, }};"
        ),
    }
    for symbol, expected_tokens in expected_operational_statements.items():
        matches = [
            statement
            for statement in operational_statements
            if statement.tokens == expected_tokens
        ]
        if len(matches) != 1:
            errors.append(
                "production operational-correspondence constant is missing, "
                f"ambiguous, or stale for {symbol}"
            )
            continue
        statement = matches[0]
        entry = {
            "path": operational_relative,
            "kind": "const",
            "symbol": symbol,
            "source_sha256": _sha256_bytes(statement.source.encode("utf-8")),
            "token_sha256": _rust_statement_token_sha256(statement),
        }
        operational_items.append(entry)
        production_items.append(entry)

    if shared_identity_entry is not None:
        production_items.append(shared_identity_entry)
    source_bindings: list[dict[str, Any]] = []
    model_by_symbol = {entry["symbol"]: entry for entry in model_symbols}
    core_by_symbol = {entry["symbol"]: entry for entry in core_items}
    verus_by_symbol = {entry["symbol"]: entry for entry in verus_items}
    operational_by_symbol = {
        entry["symbol"]: entry for entry in operational_items
    }
    operational_correspondence = {
        "id": "first_release_transition_witness_v1",
        "schema_version": 1,
        "model_source_sha256": model_source_identity,
        "action_mappings": action_mapping_entries,
        "canonical_state_encoder": operational_by_symbol.get(
            "canonical_first_release_state_bytes_v1"
        ),
        "state_digest_builder": operational_by_symbol.get(
            "production_in_flight_first_release_state_digest_v1"
        ),
        "witness_builder": operational_by_symbol.get(
            "production_in_flight_first_release_transition_witness_v1"
        ),
        "witness_authenticator": operational_by_symbol.get(
            "authenticate_production_in_flight_first_release_transition_witness_v1"
        ),
        "trace_replay_reducer": operational_by_symbol.get(
            "check_production_in_flight_first_release_replay_step_v1"
        ),
        "production_transition_checker": operational_by_symbol.get(
            "check_production_in_flight_first_release_transition"
        ),
        "replay_classification": operational_by_symbol.get(
            "ProductionInFlightFirstReleaseReplayStepV1"
        ),
        "witness_schema_version": operational_by_symbol.get(
            "PRODUCTION_IN_FLIGHT_FIRST_RELEASE_TRANSITION_WITNESS_VERSION"
        ),
        "model_source_identity": operational_by_symbol.get(
            "PRODUCTION_IN_FLIGHT_FIRST_RELEASE_TLA_SOURCE_SHA256"
        ),
        "shared_transition_kernel": core_by_symbol.get(
            "production_in_flight_first_release_transition_kernel"
        ),
        "shared_witness_binding_kernel": core_by_symbol.get(
            "production_in_flight_first_release_witness_binding_kernel"
        ),
        "verus_witness_binding_kernel": verus_by_symbol.get(
            "production_in_flight_first_release_witness_binding_kernel"
        ),
        "verus_witness_theorem": verus_by_symbol.get(
            "production_in_flight_first_release_witness_refines_named_next"
        ),
        "digest_proof_boundary": "canonical-recomputation-plus-trusted-cryptography-contract",
        "authenticated": True,
    }
    snapshot_recovery_bridge_entries: list[dict[str, Any]] = []
    for binding in PRODUCTION_SNAPSHOT_RECOVERY_BRIDGE_BINDINGS:
        item = _production_trace_unique_function(
            root_dir=root_dir,
            relative=binding["path"],
            symbol=binding["symbol"],
            impl_name=binding["impl"],
            errors=errors,
        )
        if item is None:
            continue
        item_tokens = rust_code_tokens(item.source)
        missing_tokens = [
            token
            for token in binding["required_tokens"]
            if _token_sequence_count(item_tokens, rust_code_tokens(token)) == 0
        ]
        order_error = _production_trace_ordered_token_sequence_error(
            item_tokens,
            binding.get("ordered_tokens", ()),
        )
        qualified = (
            binding["symbol"]
            if binding["impl"] is None
            else f"{binding['impl']}::{binding['symbol']}"
        )
        if missing_tokens or order_error is not None:
            detail = []
            if missing_tokens:
                detail.append(f"missing exact code tokens {missing_tokens!r}")
            if order_error is not None:
                detail.append(order_error)
            errors.append(
                "RecoverReservationSnapshot parametric bridge is incomplete at "
                f"{binding['path']}!{qualified}: " + "; ".join(detail)
            )
            continue
        entry = _production_trace_rust_item_entry(
            path=binding["path"],
            kind="fn" if binding["impl"] is None else "method",
            symbol=qualified,
            item=item,
        )
        snapshot_recovery_bridge_entries.append(entry)
        production_items.append(entry)
    if len(snapshot_recovery_bridge_entries) == len(
        PRODUCTION_SNAPSHOT_RECOVERY_BRIDGE_BINDINGS
    ):
        snapshot_recovery_bridge_by_symbol = {
            entry["symbol"]: entry for entry in snapshot_recovery_bridge_entries
        }
        source_bindings.append(
            {
                "id": "recover_reservation_snapshot_parametric_noninterference",
                "action_tags": [
                    "IN_FLIGHT_RESERVATION_ACTION_RECOVER_SNAPSHOT",
                    "IN_FLIGHT_FIRST_RELEASE_ACTION_RECOVER_RESERVATION_SNAPSHOT",
                ],
                "model_symbols": [
                    model_by_symbol.get("RecoverReservationSnapshot")
                ],
                "production_symbol": snapshot_recovery_bridge_by_symbol[
                    "IndexedReservationReplayState::check_in_flight_transition"
                ],
                "authorization_source": snapshot_recovery_bridge_by_symbol[
                    "IndexedReservationReplayState::from_replay"
                ],
                "checked_transition_consumer": snapshot_recovery_bridge_by_symbol[
                    "LaneQueueReservationJournal::consume_snapshot_replay_seal"
                ],
                "checked_transition_adapter": snapshot_recovery_bridge_by_symbol[
                    "LaneReservationSnapshotReplayReceipt::binds_reconciliation_snapshot"
                ],
                "canonical_commit_sink": snapshot_recovery_bridge_by_symbol[
                    "Queue::complete_lane_reservation_startup_reconciliation"
                ],
                "carrier_identity_projection": shared_identity_entry,
                "refinement_kernel": core_by_symbol.get(
                    "check_production_in_flight_first_release_transition"
                ),
                "verus_theorem": verus_by_symbol.get(
                    "production_in_flight_reservation_snapshot_replay_refines_composed_stutter"
                ),
                "operational_correspondence_id": operational_correspondence["id"],
                "bridge_symbols": snapshot_recovery_bridge_entries,
                "authenticated": True,
            }
        )
    for binding in PRODUCTION_TRACE_EXTRACTION_BINDINGS:
        item = _production_trace_unique_function(
            root_dir=root_dir,
            relative=binding["path"],
            symbol=binding["symbol"],
            impl_name=binding["impl"],
            errors=errors,
        )
        qualified = (
            binding["symbol"]
            if binding["impl"] is None
            else f"{binding['impl']}::{binding['symbol']}"
        )
        if item is None:
            continue
        item_tokens = rust_code_tokens(item.source)
        missing_tokens: list[str] = []
        for token in (*binding["action_tags"], *binding["additional_tokens"]):
            if _token_sequence_count(item_tokens, rust_code_tokens(token)) == 0:
                missing_tokens.append(token)

        checked_transition_source_entry = None
        checked_transition_source = binding.get("checked_transition_source")
        checked_transition_tokens = item_tokens
        if checked_transition_source is not None:
            checked_source_item = _production_trace_unique_function(
                root_dir=root_dir,
                relative=checked_transition_source["path"],
                symbol=checked_transition_source["symbol"],
                impl_name=checked_transition_source["impl"],
                errors=errors,
            )
            if checked_source_item is None:
                continue
            checked_transition_tokens = rust_code_tokens(checked_source_item.source)
            missing_checked_source_tokens = [
                token
                for token in checked_transition_source["required_tokens"]
                if _token_sequence_count(
                    checked_transition_tokens, rust_code_tokens(token)
                )
                == 0
            ]
            checked_source_order_error = _production_trace_ordered_token_sequence_error(
                checked_transition_tokens,
                checked_transition_source.get("ordered_tokens", ()),
            )
            if missing_checked_source_tokens or checked_source_order_error is not None:
                detail = []
                if missing_checked_source_tokens:
                    detail.append(
                        f"missing exact code tokens {missing_checked_source_tokens!r}"
                    )
                if checked_source_order_error is not None:
                    detail.append(checked_source_order_error)
                checked_source_qualified = (
                    checked_transition_source["symbol"]
                    if checked_transition_source["impl"] is None
                    else (
                        f"{checked_transition_source['impl']}::"
                        f"{checked_transition_source['symbol']}"
                    )
                )
                errors.append(
                    "production trace-extraction theorem missing exact checked-transition "
                    f"source {binding['id']} at {checked_transition_source['path']}!"
                    f"{checked_source_qualified}: " + "; ".join(detail)
                )
                continue
            checked_source_qualified = (
                checked_transition_source["symbol"]
                if checked_transition_source["impl"] is None
                else (
                    f"{checked_transition_source['impl']}::"
                    f"{checked_transition_source['symbol']}"
                )
            )
            checked_transition_source_entry = _production_trace_rust_item_entry(
                path=checked_transition_source["path"],
                kind="fn" if checked_transition_source["impl"] is None else "method",
                symbol=checked_source_qualified,
                item=checked_source_item,
            )
        checked_count = _token_sequence_count(
            checked_transition_tokens,
            rust_code_tokens("check_production_in_flight_first_release_transition"),
        )
        projection_count = _token_sequence_count(
            checked_transition_tokens,
            rust_code_tokens("ProductionInFlightFirstReleaseTransitionProjection"),
        )
        consumption_count = _token_sequence_count(
            item_tokens, rust_code_tokens("into_projection")
        )
        expected_count = binding["checked_transition_count"]
        expected_projection_count = (
            checked_transition_source.get("transition_projection_count", expected_count)
            if checked_transition_source is not None
            else expected_count
        )
        has_separate_consumer = binding.get("checked_transition_consumer") is not None
        if (
            missing_tokens
            or checked_count != expected_count
            or projection_count != expected_projection_count
            or (not has_separate_consumer and consumption_count < expected_count)
        ):
            detail: list[str] = []
            if missing_tokens:
                detail.append(f"missing exact code tokens {missing_tokens!r}")
            if checked_count != expected_count:
                detail.append(
                    "checked transition calls "
                    f"expected {expected_count}, found {checked_count}"
                )
            if projection_count != expected_projection_count:
                detail.append(
                    "transition projections "
                    f"expected {expected_projection_count}, found {projection_count}"
                )
            if not has_separate_consumer and consumption_count < expected_count:
                detail.append(
                    "move-only checked projection consumptions "
                    f"expected at least {expected_count}, found {consumption_count}"
                )
            errors.append(
                "production trace-extraction theorem missing authenticated binding "
                f"{binding['id']} at {binding['path']}!{qualified}: "
                + "; ".join(detail)
            )
            continue
        entry = _production_trace_rust_item_entry(
            path=binding["path"],
            kind="fn" if binding["impl"] is None else "method",
            symbol=qualified,
            item=item,
        )
        production_items.append(entry)
        if checked_transition_source_entry is not None:
            production_items.append(checked_transition_source_entry)

        supporting_source_entries = []
        supporting_sources_valid = True
        for supporting_source in binding.get("supporting_sources", ()):
            supporting_item = _production_trace_unique_function(
                root_dir=root_dir,
                relative=supporting_source["path"],
                symbol=supporting_source["symbol"],
                impl_name=supporting_source["impl"],
                errors=errors,
            )
            if supporting_item is None:
                supporting_sources_valid = False
                break
            supporting_tokens = rust_code_tokens(supporting_item.source)
            missing_supporting_tokens = [
                token
                for token in supporting_source["required_tokens"]
                if _token_sequence_count(supporting_tokens, rust_code_tokens(token)) == 0
            ]
            forbidden_supporting_tokens = [
                token
                for token in supporting_source.get("forbidden_tokens", ())
                if _token_sequence_count(supporting_tokens, rust_code_tokens(token)) != 0
            ]
            supporting_order_error = _production_trace_ordered_token_sequence_error(
                supporting_tokens,
                supporting_source.get("ordered_tokens", ()),
            )
            supporting_qualified = (
                supporting_source["symbol"]
                if supporting_source["impl"] is None
                else f"{supporting_source['impl']}::{supporting_source['symbol']}"
            )
            if (
                missing_supporting_tokens
                or forbidden_supporting_tokens
                or supporting_order_error is not None
            ):
                detail = []
                if missing_supporting_tokens:
                    detail.append(
                        f"missing exact code tokens {missing_supporting_tokens!r}"
                    )
                if forbidden_supporting_tokens:
                    detail.append(
                        "contains forbidden exact code tokens "
                        f"{forbidden_supporting_tokens!r}"
                    )
                if supporting_order_error is not None:
                    detail.append(supporting_order_error)
                errors.append(
                    "production trace-extraction theorem missing "
                    f"{supporting_source['role']} for {binding['id']} at "
                    f"{supporting_source['path']}!{supporting_qualified}: "
                    + "; ".join(detail)
                )
                supporting_sources_valid = False
                break
            supporting_entry = _production_trace_rust_item_entry(
                path=supporting_source["path"],
                kind="fn" if supporting_source["impl"] is None else "method",
                symbol=supporting_qualified,
                item=supporting_item,
            )
            supporting_source_entries.append(supporting_entry)
            production_items.append(supporting_entry)
        if not supporting_sources_valid:
            continue
        authorization_source_entry = None
        authorization_source = binding.get("authorization_source")
        if authorization_source is not None:
            authorization_source_impl = authorization_source.get("impl")
            authorization_source_qualified = (
                authorization_source["symbol"]
                if authorization_source_impl is None
                else f"{authorization_source_impl}::{authorization_source['symbol']}"
            )
            source_item = _production_trace_unique_function(
                root_dir=root_dir,
                relative=authorization_source["path"],
                symbol=authorization_source["symbol"],
                impl_name=authorization_source_impl,
                errors=errors,
            )
            if source_item is None:
                continue
            source_tokens = rust_code_tokens(source_item.source)
            missing_source_tokens = [
                token
                for token in authorization_source["required_tokens"]
                if _token_sequence_count(source_tokens, rust_code_tokens(token)) == 0
            ]
            source_order_error = _production_trace_ordered_token_sequence_error(
                source_tokens,
                authorization_source.get("ordered_tokens", ()),
            )
            if missing_source_tokens or source_order_error is not None:
                detail = (
                    f"missing exact code tokens {missing_source_tokens!r}"
                    if missing_source_tokens
                    else source_order_error
                )
                errors.append(
                    "production trace-extraction theorem missing canonical authorization "
                    f"source tokens at "
                    f"{authorization_source['path']}!{authorization_source_qualified}: "
                    f"{detail}"
                )
                continue
            authorization_source_entry = _production_trace_rust_item_entry(
                path=authorization_source["path"],
                kind="fn" if authorization_source_impl is None else "method",
                symbol=authorization_source_qualified,
                item=source_item,
            )
            production_items.append(authorization_source_entry)
        checked_transition_consumer_entry = None
        checked_transition_consumer = binding.get("checked_transition_consumer")
        if checked_transition_consumer is not None:
            checked_transition_consumer_impl = checked_transition_consumer.get("impl")
            checked_transition_consumer_qualified = (
                checked_transition_consumer["symbol"]
                if checked_transition_consumer_impl is None
                else (
                    f"{checked_transition_consumer_impl}::"
                    f"{checked_transition_consumer['symbol']}"
                )
            )
            consumer_item = _production_trace_unique_function(
                root_dir=root_dir,
                relative=checked_transition_consumer["path"],
                symbol=checked_transition_consumer["symbol"],
                impl_name=checked_transition_consumer_impl,
                errors=errors,
            )
            if consumer_item is None:
                continue
            consumer_tokens = rust_code_tokens(consumer_item.source)
            missing_consumer_tokens = [
                token
                for token in checked_transition_consumer["required_tokens"]
                if _token_sequence_count(consumer_tokens, rust_code_tokens(token)) == 0
            ]
            consumer_count = _token_sequence_count(
                consumer_tokens, rust_code_tokens("into_projection")
            )
            consumer_order_error = _production_trace_ordered_token_sequence_error(
                consumer_tokens,
                checked_transition_consumer.get("ordered_tokens", ()),
            )
            if (
                missing_consumer_tokens
                or consumer_count < expected_count
                or consumer_order_error is not None
            ):
                detail = []
                if missing_consumer_tokens:
                    detail.append(
                        f"missing exact code tokens {missing_consumer_tokens!r}"
                    )
                if consumer_count < expected_count:
                    detail.append(
                        "move-only checked projection consumptions "
                        f"expected at least {expected_count}, found {consumer_count}"
                    )
                if consumer_order_error is not None:
                    detail.append(consumer_order_error)
                errors.append(
                    "production trace-extraction theorem missing move-only consumer "
                    f"{binding['id']} at {checked_transition_consumer['path']}!"
                    f"{checked_transition_consumer_qualified}: "
                    + "; ".join(detail)
                )
                continue
            checked_transition_consumer_entry = _production_trace_rust_item_entry(
                path=checked_transition_consumer["path"],
                kind="fn" if checked_transition_consumer_impl is None else "method",
                symbol=checked_transition_consumer_qualified,
                item=consumer_item,
            )
            production_items.append(checked_transition_consumer_entry)
        checked_transition_adapter_entry = None
        checked_transition_adapter = binding.get("checked_transition_adapter")
        if checked_transition_adapter is not None:
            checked_transition_adapter_impl = checked_transition_adapter.get("impl")
            checked_transition_adapter_qualified = (
                checked_transition_adapter["symbol"]
                if checked_transition_adapter_impl is None
                else (
                    f"{checked_transition_adapter_impl}::"
                    f"{checked_transition_adapter['symbol']}"
                )
            )
            adapter_item = _production_trace_unique_function(
                root_dir=root_dir,
                relative=checked_transition_adapter["path"],
                symbol=checked_transition_adapter["symbol"],
                impl_name=checked_transition_adapter_impl,
                errors=errors,
            )
            if adapter_item is None:
                continue
            adapter_tokens = rust_code_tokens(adapter_item.source)
            missing_adapter_tokens = [
                token
                for token in checked_transition_adapter["required_tokens"]
                if _token_sequence_count(adapter_tokens, rust_code_tokens(token)) == 0
            ]
            adapter_order_error = _production_trace_ordered_token_sequence_error(
                adapter_tokens,
                checked_transition_adapter.get("ordered_tokens", ()),
            )
            if missing_adapter_tokens or adapter_order_error is not None:
                detail = (
                    f"missing exact code tokens {missing_adapter_tokens!r}"
                    if missing_adapter_tokens
                    else adapter_order_error
                )
                errors.append(
                    "production trace-extraction theorem missing move-only State "
                    f"commit adapter {binding['id']} at "
                    f"{checked_transition_adapter['path']}!"
                    f"{checked_transition_adapter_qualified}: {detail}"
                )
                continue
            checked_transition_adapter_entry = _production_trace_rust_item_entry(
                path=checked_transition_adapter["path"],
                kind="fn" if checked_transition_adapter_impl is None else "method",
                symbol=checked_transition_adapter_qualified,
                item=adapter_item,
            )
            production_items.append(checked_transition_adapter_entry)
        commit_sink_entry = None
        commit_sink = binding.get("commit_sink")
        if commit_sink is not None:
            commit_sink_impl = commit_sink.get("impl")
            commit_sink_symbol = (
                commit_sink["symbol"]
                if commit_sink_impl is None
                else f"{commit_sink_impl}::{commit_sink['symbol']}"
            )
            sink_item = _production_trace_unique_function(
                root_dir=root_dir,
                relative=commit_sink["path"],
                symbol=commit_sink["symbol"],
                impl_name=commit_sink_impl,
                errors=errors,
            )
            if sink_item is None:
                continue
            sink_tokens = rust_code_tokens(sink_item.source)
            missing_sink_tokens = [
                token
                for token in commit_sink["required_tokens"]
                if _token_sequence_count(sink_tokens, rust_code_tokens(token)) == 0
            ]
            sink_order_error = _production_trace_ordered_token_sequence_error(
                sink_tokens,
                commit_sink.get("ordered_tokens", ()),
            )
            if missing_sink_tokens or sink_order_error is not None:
                detail = (
                    f"missing exact code tokens {missing_sink_tokens!r}"
                    if missing_sink_tokens
                    else sink_order_error
                )
                errors.append(
                    "production trace-extraction theorem missing canonical commit "
                    f"sink tokens at "
                    f"{commit_sink['path']}!{commit_sink_symbol}: {detail}"
                )
                continue
            commit_sink_entry = _production_trace_rust_item_entry(
                path=commit_sink["path"],
                kind="fn" if commit_sink_impl is None else "method",
                symbol=commit_sink_symbol,
                item=sink_item,
            )
            production_items.append(commit_sink_entry)
        missing_model_actions = [
            action
            for action in binding["model_actions"]
            if action not in model_by_symbol
        ]
        if missing_model_actions:
            errors.append(
                "production trace-extraction theorem cannot bind missing model "
                f"operators {missing_model_actions!r} for {binding['id']}"
            )
            continue
        source_bindings.append(
            {
                "id": binding["id"],
                "action_tags": list(binding["action_tags"]),
                "model_symbols": [
                    model_by_symbol[action]
                    for action in binding["model_actions"]
                ],
                "production_symbol": entry,
                "checked_transition_source": checked_transition_source_entry,
                "supporting_sources": supporting_source_entries,
                "authorization_source": authorization_source_entry,
                "checked_transition_consumer": checked_transition_consumer_entry,
                "checked_transition_adapter": checked_transition_adapter_entry,
                "canonical_commit_sink": commit_sink_entry,
                "carrier_identity_projection": shared_identity_entry,
                "refinement_kernel": core_by_symbol.get(
                    "check_production_in_flight_first_release_transition"
                ),
                "verus_theorem": verus_by_symbol.get(
                    "production_in_flight_first_release_transition_refines_named_next"
                ),
                "operational_correspondence_id": operational_correspondence["id"],
                "authenticated": True,
            }
        )

    try:
        multilane_checker = _load_multilane_model_checker()
        # The strict formal launcher has already run the complete multilane
        # structural checker. Recompute its exact source manifest here and
        # independently recheck the theorem seams above; replaying the
        # entire unrelated closure inventory would make certificate validation
        # needlessly unbounded.
        multilane_manifest = multilane_checker.source_manifest_sha256(root_dir)
    except (OSError, UnicodeDecodeError, ValueError, RuntimeError) as error:
        errors.append(f"could not authenticate multilane source bindings: {error}")
        multilane_manifest = None

    if errors:
        raise ValueError("\n".join(errors))

    fixed_relative = "formal/sumeragi_v2/inflight_first_release_fixed.cfg"
    checker_relative = "scripts/formal/check_sumeragi_v2_multilane_models.py"
    model_sources = []
    for relative, label in (
        (model_relative, "in-flight TLA+ model"),
        (fixed_relative, "in-flight positive model config"),
        (bindings_relative, "multilane source-binding ledger"),
        (checker_relative, "multilane source-binding checker"),
    ):
        payload = _bounded_regular_file_bytes(
            root_dir / relative,
            label=label,
            maximum_bytes=PRODUCTION_TRACE_EXTRACTION_COMPONENT_MAX_BYTES,
        )
        model_sources.append(
            {
                "path": relative,
                "sha256": _sha256_bytes(payload),
                "size_bytes": len(payload),
            }
        )
    return {
        "multilane_source_manifest_sha256": multilane_manifest,
        "model_sources": model_sources,
        "model_symbols": model_symbols,
        "refinement_symbols": core_items,
        "production_symbols": production_items,
        "verus_theorems": verus_items,
        "operational_correspondence": operational_correspondence,
        "source_bindings": source_bindings,
    }


def _production_trace_ordered_token_sequence_error(
    source_tokens: Sequence[str], required_tokens: Sequence[str]
) -> str | None:
    """Return a fail-closed error unless each token sequence occurs once in order."""

    cursor = -1
    for required in required_tokens:
        needle = rust_code_tokens(required)
        positions = [
            index
            for index in range(len(source_tokens) - len(needle) + 1)
            if tuple(source_tokens[index : index + len(needle)]) == tuple(needle)
        ]
        if len(positions) != 1:
            return (
                f"ordered code token {required!r} must occur exactly once, "
                f"found {len(positions)}"
            )
        if positions[0] <= cursor:
            return f"ordered code token {required!r} moved before its predecessor"
        cursor = positions[0]
    return None


def _production_trace_extraction_action_partition_errors() -> list[str]:
    """Require concrete bindings plus explicit debt to cover the whole model."""

    required = PRODUCTION_TRACE_EXTRACTION_REQUIRED_MODEL_ACTIONS
    open_actions = PRODUCTION_TRACE_EXTRACTION_OPEN_MODEL_ACTIONS
    bound_actions = tuple(
        action
        for binding in PRODUCTION_TRACE_EXTRACTION_BINDINGS
        for action in binding["model_actions"]
    )
    required_set = set(required)
    open_set = set(open_actions)
    bound_set = set(bound_actions)
    errors: list[str] = []
    if len(required_set) != len(required):
        errors.append(
            "production trace-extraction required model-action inventory "
            "contains duplicates"
        )
    if len(open_set) != len(open_actions):
        errors.append(
            "production trace-extraction open model-action inventory contains "
            "duplicates"
        )
    overlap = [
        action
        for action in required
        if action in open_set and action in bound_set
    ]
    missing = [action for action in required if action not in open_set | bound_set]
    unexpected = sorted((open_set | bound_set) - required_set)
    if overlap or missing or unexpected:
        errors.append(
            "production trace-extraction bindings and explicit open actions do "
            "not partition the exact model-action inventory: "
            f"overlap={overlap!r}, missing={missing!r}, unexpected={unexpected!r}"
        )
    return errors


def _production_trace_extraction_ledger_dependency_snapshot(
    ledger: dict[str, Any],
) -> list[dict[str, str]]:
    """Return the exact proved/trusted ledger slice used by this theorem."""

    obligations = ledger.get("obligations")
    if not isinstance(obligations, list):
        raise ValueError(
            "production trace-extraction evidence requires a proof obligation array"
        )
    by_id: dict[str, dict[str, Any]] = {}
    for obligation in obligations:
        if not isinstance(obligation, dict):
            continue
        obligation_id = obligation.get("id")
        if not _nonempty_string(obligation_id):
            continue
        if obligation_id in by_id:
            raise ValueError(
                "production trace-extraction ledger dependency inventory contains "
                f"duplicate obligation {obligation_id}"
            )
        by_id[obligation_id] = obligation

    snapshot: list[dict[str, str]] = []
    for obligation_id, expected_status in (
        PRODUCTION_TRACE_EXTRACTION_LEDGER_DEPENDENCIES
    ):
        obligation = by_id.get(obligation_id)
        if obligation is None:
            raise ValueError(
                "production trace-extraction ledger dependency is missing: "
                f"{obligation_id}"
            )
        observed_status = obligation.get("status")
        if observed_status != expected_status:
            raise ValueError(
                "production trace-extraction ledger dependency status drifted: "
                f"{obligation_id} expected {expected_status}, found "
                f"{observed_status!r}"
            )
        snapshot.append({"id": obligation_id, "status": expected_status})
    return snapshot


def build_production_trace_extraction_evidence(
    ledger: dict[str, Any],
    *,
    tlaps_evidence: dict[str, Any],
    verus_evidence: dict[str, Any],
    cross_tool_evidence: dict[str, Any] | None,
    artifacts: ProductionTraceExtractionArtifactPaths,
    root_dir: Path = ROOT_DIR,
    formal_dir: Path = FORMAL_DIR,
) -> dict[str, Any]:
    """Build the exact source- and backend-bound production theorem certificate."""

    ledger_dependencies = _production_trace_extraction_ledger_dependency_snapshot(
        ledger
    )
    partition_errors = _production_trace_extraction_action_partition_errors()
    if partition_errors:
        raise ValueError("\n".join(partition_errors))
    if PRODUCTION_TRACE_EXTRACTION_OPEN_MODEL_ACTIONS:
        raise ValueError(
            "production trace-extraction evidence cannot be certified while "
            "model actions remain unextracted: "
            + ", ".join(PRODUCTION_TRACE_EXTRACTION_OPEN_MODEL_ACTIONS)
        )
    if not all(
        isinstance(value, dict) for value in (tlaps_evidence, verus_evidence)
    ):
        raise ValueError(
            "production trace-extraction evidence requires TLAPS and Verus "
            "evidence objects"
        )
    formal_manifest = tlaps_evidence.get("source_manifest")
    if not isinstance(formal_manifest, dict) or not _nonempty_string(
        formal_manifest.get("sha256")
    ):
        raise ValueError("TLAPS evidence lacks its formal source manifest")
    workspace_manifest = verus_evidence.get("source_manifest_sha256")
    if not _nonempty_string(workspace_manifest):
        raise ValueError("Verus evidence lacks its workspace source manifest")
    tlaps_ledger_sha256 = tlaps_evidence.get("ledger_sha256")
    if not _nonempty_string(tlaps_ledger_sha256):
        raise ValueError("TLAPS evidence lacks its exact proof-ledger digest")
    component_evidence = {
        "tlaps_sha256": _canonical_json_sha256(tlaps_evidence),
        "verus_sha256": _canonical_json_sha256(verus_evidence),
    }
    if cross_tool_evidence is not None:
        if not isinstance(cross_tool_evidence, dict):
            raise ValueError("cross-tool evidence must be an object when supplied")
        source_manifests = cross_tool_evidence.get("source_manifests")
        if source_manifests != {
            "formal_sha256": formal_manifest["sha256"],
            "workspace_sha256": workspace_manifest,
        }:
            raise ValueError(
                "cross-tool evidence does not link the exact formal and workspace "
                "manifests"
            )
        if cross_tool_evidence.get("ledger_sha256") != tlaps_ledger_sha256:
            raise ValueError(
                "cross-tool evidence does not link the exact proof ledger"
            )
        if cross_tool_evidence.get("component_evidence") != component_evidence:
            raise ValueError(
                "cross-tool evidence does not link the exact backend evidence"
            )
    source_snapshot = _production_trace_extraction_source_snapshot(
        root_dir=root_dir, formal_dir=formal_dir
    )
    artifact_entries = [
        _production_trace_artifact_entry("proof_ledger", artifacts.ledger),
        _production_trace_artifact_entry("tlaps_evidence", artifacts.evidence),
        _production_trace_artifact_entry("verus_evidence", artifacts.verus_evidence),
        _production_trace_artifact_entry("verus_log", artifacts.verus_log),
    ]
    if artifacts.cross_tool_evidence is not None:
        artifact_entries.append(
            _production_trace_artifact_entry(
                "cross_tool_evidence", artifacts.cross_tool_evidence
            )
        )
    return {
        "schema_version": PRODUCTION_TRACE_EXTRACTION_EVIDENCE_SCHEMA_VERSION,
        "certificate_type": "production_trace_extraction_theorem",
        "theorem": PRODUCTION_TRACE_EXTRACTION_THEOREM,
        "canonical_encoding": PRODUCTION_TRACE_EXTRACTION_CANONICAL_ENCODING,
        "backend_verification": True,
        "workspace_source_manifest_sha256": workspace_manifest,
        "formal_source_manifest_sha256": formal_manifest["sha256"],
        "multilane_source_manifest_sha256": source_snapshot[
            "multilane_source_manifest_sha256"
        ],
        "artifacts": artifact_entries,
        "model_sources": source_snapshot["model_sources"],
        "model_symbols": source_snapshot["model_symbols"],
        "refinement_symbols": source_snapshot["refinement_symbols"],
        "production_symbols": source_snapshot["production_symbols"],
        "verus_theorems": source_snapshot["verus_theorems"],
        "operational_correspondence": source_snapshot[
            "operational_correspondence"
        ],
        "source_bindings": source_snapshot["source_bindings"],
        "proof_linkage": {
            "ledger_document_sha256": _canonical_json_sha256(ledger),
            "tlaps_document_sha256": _canonical_json_sha256(tlaps_evidence),
            "verus_document_sha256": _canonical_json_sha256(verus_evidence),
            "cross_tool_document_sha256": (
                None
                if cross_tool_evidence is None
                else _canonical_json_sha256(cross_tool_evidence)
            ),
            "cross_tool_ledger_sha256": (
                None
                if cross_tool_evidence is None
                else cross_tool_evidence["ledger_sha256"]
            ),
            "component_evidence": component_evidence,
            "verus_log_sha256": verus_evidence.get("log_sha256"),
            "multilane_dependency_completion": True,
            "multilane_ledger_dependencies": ledger_dependencies,
            "global_machine_checked_completion": ledger.get(
                "machine_checked_completion"
            ),
        },
    }


def _production_trace_extraction_evidence_errors(
    ledger: dict[str, Any],
    observed: dict[str, Any] | None,
    *,
    tlaps_evidence: dict[str, Any] | None,
    verus_evidence: dict[str, Any] | None,
    cross_tool_evidence: dict[str, Any] | None,
    artifacts: ProductionTraceExtractionArtifactPaths | None,
    root_dir: Path = ROOT_DIR,
    formal_dir: Path = FORMAL_DIR,
) -> list[str]:
    if observed is None:
        return []
    if not isinstance(observed, dict):
        return ["production trace-extraction evidence must be a JSON object"]
    if artifacts is None:
        return ["production trace-extraction evidence lacks exact artifact paths"]
    if not all(
        isinstance(value, dict) for value in (tlaps_evidence, verus_evidence)
    ):
        return [
            "production trace-extraction evidence requires linked TLAPS and "
            "Verus evidence"
        ]
    try:
        expected = build_production_trace_extraction_evidence(
            ledger,
            tlaps_evidence=tlaps_evidence,
            verus_evidence=verus_evidence,
            cross_tool_evidence=cross_tool_evidence,
            artifacts=artifacts,
            root_dir=root_dir,
            formal_dir=formal_dir,
        )
    except (OSError, UnicodeDecodeError, ValueError) as error:
        return [f"production trace-extraction theorem cannot be authenticated: {error}"]
    mismatch = _first_json_mismatch(expected, observed)
    if mismatch is not None:
        return [
            "production trace-extraction evidence does not match the canonical "
            f"current theorem certificate at {mismatch}"
        ]
    return []
