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
    expected_context = None if impl_name is None else (("impl", impl_name),)
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

    ordered_actions: list[str] = []
    for binding in PRODUCTION_TRACE_EXTRACTION_BINDINGS:
        for action in binding["model_actions"]:
            if action not in ordered_actions:
                ordered_actions.append(action)
    model_symbols: list[dict[str, Any]] = []
    for symbol in (
        *ordered_actions,
        "Next",
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

    core_relative = "crates/iroha_core/src/sumeragi/v2_core/refinement.rs"
    core_items: list[dict[str, Any]] = []
    for symbol in (
        "check_production_in_flight_first_release_transition",
        "production_in_flight_first_release_terminal_owner",
    ):
        item = _production_trace_unique_function(
            root_dir=root_dir,
            relative=core_relative,
            symbol=symbol,
            impl_name=None,
            errors=errors,
        )
        if item is not None:
            core_items.append(
                _production_trace_rust_item_entry(
                    path=core_relative, kind="fn", symbol=symbol, item=item
                )
            )
    try:
        core_source = _bounded_regular_file_bytes(
            root_dir / core_relative,
            label="production first-release refinement kernel",
            maximum_bytes=PRODUCTION_TRACE_EXTRACTION_COMPONENT_MAX_BYTES,
        ).decode("utf-8")
    except (UnicodeDecodeError, ValueError) as error:
        errors.append(str(error))
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

    verus_relative = (
        "crates/iroha_sumeragi_core/src/verus_proofs/"
        "application_release_proofs.rs"
    )
    verus_items: list[dict[str, Any]] = []
    for symbol in (
        "production_in_flight_first_release_transition_refines_named_next",
        "production_in_flight_first_release_snapshot_recovery_is_stutter",
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

    production_items: list[dict[str, Any]] = []
    source_bindings: list[dict[str, Any]] = []
    model_by_symbol = {entry["symbol"]: entry for entry in model_symbols}
    core_by_symbol = {entry["symbol"]: entry for entry in core_items}
    verus_by_symbol = {entry["symbol"]: entry for entry in verus_items}
    for binding in PRODUCTION_TRACE_EXTRACTION_BINDINGS:
        item = _production_trace_unique_function(
            root_dir=root_dir,
            relative=binding["path"],
            symbol=binding["symbol"],
            impl_name=binding["impl"],
            errors=errors,
        )
        qualified = f"{binding['impl']}::{binding['symbol']}"
        if item is None:
            continue
        item_tokens = rust_code_tokens(item.source)
        missing_tokens: list[str] = []
        for token in (*binding["action_tags"], *binding["additional_tokens"]):
            if _token_sequence_count(item_tokens, rust_code_tokens(token)) == 0:
                missing_tokens.append(token)
        checked_count = _token_sequence_count(
            item_tokens,
            rust_code_tokens("check_production_in_flight_first_release_transition"),
        )
        projection_count = _token_sequence_count(
            item_tokens,
            rust_code_tokens("ProductionInFlightFirstReleaseTransitionProjection"),
        )
        consumption_count = _token_sequence_count(
            item_tokens, rust_code_tokens("into_projection")
        )
        expected_count = binding["checked_transition_count"]
        if (
            missing_tokens
            or checked_count != expected_count
            or projection_count != expected_count
            or consumption_count < expected_count
        ):
            detail: list[str] = []
            if missing_tokens:
                detail.append(f"missing exact code tokens {missing_tokens!r}")
            if checked_count != expected_count:
                detail.append(
                    "checked transition calls "
                    f"expected {expected_count}, found {checked_count}"
                )
            if projection_count != expected_count:
                detail.append(
                    "transition projections "
                    f"expected {expected_count}, found {projection_count}"
                )
            if consumption_count < expected_count:
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
            path=binding["path"], kind="method", symbol=qualified, item=item
        )
        production_items.append(entry)
        commit_sink_entry = None
        commit_sink = binding.get("commit_sink")
        if commit_sink is not None:
            sink_item = _production_trace_unique_function(
                root_dir=root_dir,
                relative=commit_sink["path"],
                symbol=commit_sink["symbol"],
                impl_name=commit_sink["impl"],
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
            if missing_sink_tokens:
                errors.append(
                    "production trace-extraction theorem missing canonical commit "
                    f"sink tokens {missing_sink_tokens!r} at "
                    f"{commit_sink['path']}!{commit_sink['impl']}::"
                    f"{commit_sink['symbol']}"
                )
                continue
            commit_sink_entry = _production_trace_rust_item_entry(
                path=commit_sink["path"],
                kind="method",
                symbol=f"{commit_sink['impl']}::{commit_sink['symbol']}",
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
                "canonical_commit_sink": commit_sink_entry,
                "refinement_kernel": core_by_symbol.get(
                    "check_production_in_flight_first_release_transition"
                ),
                "verus_theorem": verus_by_symbol.get(
                    "production_in_flight_first_release_transition_refines_named_next"
                ),
                "authenticated": True,
            }
        )

    try:
        multilane_checker = _load_multilane_model_checker()
        # The strict formal launcher has already run the complete multilane
        # structural checker. Recompute its exact source manifest here and
        # independently recheck the four theorem seams above; replaying the
        # entire unrelated closure inventory would make certificate validation
        # needlessly unbounded.
        multilane_manifest = multilane_checker.source_manifest_sha256(root_dir)
    except (OSError, UnicodeDecodeError, ValueError, RuntimeError) as error:
        errors.append(f"could not authenticate multilane source bindings: {error}")
        multilane_manifest = None

    if errors:
        raise ValueError("\n".join(errors))

    fixed_relative = "formal/sumeragi_v2/inflight_first_release_fixed.cfg"
    bindings_relative = "formal/sumeragi_v2/multilane_source_bindings.json"
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
        "source_bindings": source_bindings,
    }


def build_production_trace_extraction_evidence(
    ledger: dict[str, Any],
    *,
    tlaps_evidence: dict[str, Any],
    verus_evidence: dict[str, Any],
    cross_tool_evidence: dict[str, Any],
    artifacts: ProductionTraceExtractionArtifactPaths,
    root_dir: Path = ROOT_DIR,
    formal_dir: Path = FORMAL_DIR,
) -> dict[str, Any]:
    """Build the exact source- and backend-bound production theorem certificate."""

    if ledger.get("machine_checked_completion") is not True:
        raise ValueError(
            "production trace-extraction evidence requires "
            "machine_checked_completion=true"
        )
    if not all(
        isinstance(value, dict)
        for value in (tlaps_evidence, verus_evidence, cross_tool_evidence)
    ):
        raise ValueError(
            "production trace-extraction evidence requires TLAPS, Verus, and "
            "cross-tool evidence objects"
        )
    formal_manifest = tlaps_evidence.get("source_manifest")
    if not isinstance(formal_manifest, dict) or not _nonempty_string(
        formal_manifest.get("sha256")
    ):
        raise ValueError("TLAPS evidence lacks its formal source manifest")
    workspace_manifest = verus_evidence.get("source_manifest_sha256")
    if not _nonempty_string(workspace_manifest):
        raise ValueError("Verus evidence lacks its workspace source manifest")
    source_manifests = cross_tool_evidence.get("source_manifests")
    if source_manifests != {
        "formal_sha256": formal_manifest["sha256"],
        "workspace_sha256": workspace_manifest,
    }:
        raise ValueError(
            "cross-tool evidence does not link the exact formal and workspace "
            "manifests"
        )
    if cross_tool_evidence.get("ledger_sha256") != _canonical_json_sha256(ledger):
        raise ValueError("cross-tool evidence does not link the exact proof ledger")
    component_evidence = cross_tool_evidence.get("component_evidence")
    if component_evidence != {
        "tlaps_sha256": _canonical_json_sha256(tlaps_evidence),
        "verus_sha256": _canonical_json_sha256(verus_evidence),
    }:
        raise ValueError("cross-tool evidence does not link the exact backend evidence")

    source_snapshot = _production_trace_extraction_source_snapshot(
        root_dir=root_dir, formal_dir=formal_dir
    )
    artifact_entries = [
        _production_trace_artifact_entry("proof_ledger", artifacts.ledger),
        _production_trace_artifact_entry("tlaps_evidence", artifacts.evidence),
        _production_trace_artifact_entry("verus_evidence", artifacts.verus_evidence),
        _production_trace_artifact_entry("verus_log", artifacts.verus_log),
        _production_trace_artifact_entry(
            "cross_tool_evidence", artifacts.cross_tool_evidence
        ),
    ]
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
        "source_bindings": source_snapshot["source_bindings"],
        "proof_linkage": {
            "ledger_document_sha256": _canonical_json_sha256(ledger),
            "tlaps_document_sha256": _canonical_json_sha256(tlaps_evidence),
            "verus_document_sha256": _canonical_json_sha256(verus_evidence),
            "cross_tool_document_sha256": _canonical_json_sha256(
                cross_tool_evidence
            ),
            "cross_tool_ledger_sha256": cross_tool_evidence["ledger_sha256"],
            "cross_tool_component_evidence": component_evidence,
            "verus_log_sha256": verus_evidence.get("log_sha256"),
            "machine_checked_completion": True,
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
        isinstance(value, dict)
        for value in (tlaps_evidence, verus_evidence, cross_tool_evidence)
    ):
        return [
            "production trace-extraction evidence requires linked TLAPS, Verus, "
            "and cross-tool evidence"
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


