@dataclass(frozen=True)
class ProductionLivenessHelperSeal:
    """One whole-item seal for production code owned by a release regression."""

    source: str
    item: str
    item_token_sha256: str
    kind: str = "item"
    brace_context: tuple[tuple[str, ...], ...] = ()


_PRODUCTION_LIVENESS_HELPER_SEALS = (
    ProductionLivenessHelperSeal(
        "crates/iroha_core/src/kura.rs",
        "AutonomousLaneRetiredAttempt",
        "fd49299fd0080279e3a51be3f8a16aa9ede49635c608d033913d8e4ca8912b76",
        "struct",
    ),
    ProductionLivenessHelperSeal(
        "crates/iroha_core/src/kura.rs",
        "read_autonomous_lane_retired_attempt",
        "cd7120b23a8a22f1704b7ead6ca258242faba7e4c491b599f88d2b297a61349d",
        brace_context=(("impl", "Kura"),),
    ),
    ProductionLivenessHelperSeal(
        "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
        "outbound_lane_message_predecessor_is_ready",
        "e1adbe51e47c7d2e3629d63279b377473b9140e49e6a1b1970c9483818ebcf4d",
        brace_context=(("impl", "V2LaneWorkAdapter"),),
    ),
    ProductionLivenessHelperSeal(
        "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
        "historical_raw_proposal_can_solicit_certificate",
        "f2ce1439fa32ab3dd5df854d4cf6837cab71da5ca74f9cafb5ef040d43b3a8c9",
        brace_context=(("impl", "V2LaneWorkAdapter"),),
    ),
    ProductionLivenessHelperSeal(
        "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
        "proposal_can_be_transported",
        "bb0f15aaf294aa9667b9cf86bd4e1afc86af8d0044d506d50522b30d40fb9900",
        brace_context=(("impl", "V2LaneWorkAdapter"),),
    ),
    ProductionLivenessHelperSeal(
        "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
        "proposal_predecessor_is_ready_for_progress",
        "af90f5ebe15136bfc8255dc8d1a8aeac7101d4226c79ef8a94e4821eeeb0d78a",
        brace_context=(("impl", "V2LaneWorkAdapter"),),
    ),
    ProductionLivenessHelperSeal(
        "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
        "proposal_can_progress",
        "eed1b8bf381b8ffe749d5ac1080d5bb23620b70d95b86ee75303ffec9606ffd0",
        brace_context=(("impl", "V2LaneWorkAdapter"),),
    ),
    ProductionLivenessHelperSeal(
        "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
        "historical_lane_recovery_message_is_authorized",
        "a9a193ee69dca20a51183279bc12e98480cad6c99dfbb7f22f81aa6b01458c7f",
        brace_context=(("impl", "V2LaneWorkAdapter"),),
    ),
    ProductionLivenessHelperSeal(
        "crates/iroha_core/src/sumeragi/v2_worker.rs",
        "autonomous_lane_output_matches_payload_identity",
        "6ad31433c9592ac390c299fb4bc0d7174b9dc36cde05a8168f208482a6ac9d18",
    ),
    ProductionLivenessHelperSeal(
        "crates/iroha_core/src/sumeragi/v2_worker.rs",
        "autonomous_lane_output_has_exact_retirement_source",
        "f160be0e0bcce5a09813d9b1ce971c361991bff4ac9a33469a5130bbd11b80f6",
    ),
    ProductionLivenessHelperSeal(
        "crates/iroha_core/src/sumeragi/v2_worker.rs",
        "autonomous_lane_output_has_durable_reconstruction_source",
        "0d806079c721d886405ad801b9fcb145b81387c2b9bc757534c82a4295a60e82",
    ),
)

_PRODUCTION_LIVENESS_HELPER_FIXTURE_SOURCES = {
    "crates/iroha_core/src/kura.rs": (
        "crates/iroha_core/src/kura/autonomous_retired_attempt.rs"
    ),
    "crates/iroha_core/src/sumeragi/v2_lane_work.rs": (
        "crates/iroha_core/src/sumeragi/v2_lane_work.rs"
    ),
    "crates/iroha_core/src/sumeragi/v2_worker.rs": (
        "crates/iroha_core/src/sumeragi/v2_worker/"
        "autonomous_lane_output_reconstruction.rs"
    ),
}


def _production_liveness_helper_source_seal_errors(
    repo_root: Path = ROOT_DIR,
) -> list[str]:
    """Bind release-regression helpers to authenticated whole Rust items."""

    errors: list[str] = []
    sources: dict[str, tuple[Path, str]] = {}
    for relative in dict.fromkeys(
        seal.source for seal in _PRODUCTION_LIVENESS_HELPER_SEALS
    ):
        logical_path = repo_root / relative
        if repo_root.resolve() == ROOT_DIR.resolve():
            path, source = _read_reviewed_rust_source(
                repo_root,
                relative,
                errors,
                "production liveness helper source",
            )
        else:
            # Synthetic mutation roots are not Git worktrees. Read only the
            # reviewed fragment under mutation; the real repository path above
            # remains authenticated by the recursive staged-source resolver.
            fixture_relative = _PRODUCTION_LIVENESS_HELPER_FIXTURE_SOURCES[relative]
            fixture_path = repo_root / fixture_relative
            path = logical_path
            if not fixture_path.is_file() or fixture_path.is_symlink():
                errors.append(
                    f"{fixture_path}: production liveness helper fixture must "
                    "be a regular non-symlink file"
                )
                source = ""
            else:
                try:
                    source = fixture_path.read_text(encoding="utf-8")
                except (OSError, UnicodeDecodeError) as error:
                    errors.append(
                        f"{fixture_path}: cannot read production liveness "
                        f"helper fixture: {error}"
                    )
                    source = ""
        sources[relative] = (path, source)

    for seal in _PRODUCTION_LIVENESS_HELPER_SEALS:
        path, source = sources[seal.source]
        if seal.kind == "struct":
            matches = rust_struct_items(source, seal.item)
            if len(matches) != 1:
                errors.append(
                    f"{path}: require exactly one real Rust struct item named "
                    f"{seal.item}; found {len(matches)}"
                )
                item = None
            else:
                item = matches[0]
        elif seal.kind == "item":
            item = _require_rust_item(path, source, seal.item, errors)
        else:
            errors.append(
                "internal production liveness helper seal has unsupported "
                f"kind {seal.kind!r} for {seal.source}!{seal.item}"
            )
            continue
        description = f"production liveness helper {seal.item}"
        _require_rust_item_context(
            path,
            item,
            seal.brace_context,
            description,
            errors,
        )
        _require_rust_item_token_sha256(
            path,
            item,
            seal.item_token_sha256,
            description,
            errors,
        )
    return errors


def _relative_to_root(path: Path, root_dir: Path = ROOT_DIR) -> str:
    try:
        return path.resolve().relative_to(root_dir.resolve()).as_posix()
    except ValueError as error:
        raise ValueError(f"path is outside the repository: {path}") from error


FORMAL_EVIDENCE_LOGICAL_ROOT = Path("formal/sumeragi_v2")


def _formal_evidence_logical_path(*parts: str) -> str:
    return FORMAL_EVIDENCE_LOGICAL_ROOT.joinpath(*parts).as_posix()


def _formal_evidence_physical_path(
    logical_path: str, root_dir: Path = ROOT_DIR
) -> Path:
    logical = Path(logical_path)
    try:
        relative = logical.relative_to(FORMAL_EVIDENCE_LOGICAL_ROOT)
    except ValueError as error:
        raise ValueError(
            f"formal evidence path escapes its logical root: {logical_path}"
        ) from error
    external = os.environ.get("SUMERAGI_V2_FORMAL_EVIDENCE_DIR")
    base = (
        Path(external)
        if external is not None
        else root_dir / FORMAL_EVIDENCE_LOGICAL_ROOT
    )
    return base / relative


def _production_liveness_release_inventory_errors(
    repo_root: Path = ROOT_DIR,
) -> list[str]:
    """Seal the production-regression inventory and its default-feature scope."""

    errors: list[str] = []
    release_path = repo_root / "scripts" / "run_sumeragi_v2_release_gates.sh"
    if not release_path.is_file() or release_path.is_symlink():
        return [
            f"{release_path}: production liveness release runner must be a regular file"
        ]
    source = release_path.read_text(encoding="utf-8")
    errors.extend(_production_liveness_release_inventory_guard_errors(repo_root))
    errors.extend(_production_liveness_helper_source_seal_errors(repo_root))

    def shell_array(name: str) -> list[str]:
        marker = f"{name}=(\n"
        if source.count(marker) != 1:
            errors.append(
                f"{release_path}: release runner must contain one canonical {name} array"
            )
            return []
        tail = source.split(marker, 1)[1]
        if "\n)" not in tail:
            errors.append(f"{release_path}: release runner has unterminated {name} array")
            return []
        body = tail.split("\n)", 1)[0]
        return [
            line.strip()
            for line in body.splitlines()
            if line.strip() and not line.lstrip().startswith("#")
        ]

    canonical_grouped_sdk_suites = (
        ("openapi", 7),
        ("python", 63),
        ("javascript", 61),
        ("swift", 5),
        ("kotlin", 7),
        ("java", 6),
    )
    def indented_shell_array(name: str) -> list[str]:
        matches = re.findall(
            rf"^  {re.escape(name)}=\(\n((?:    [^\n]*\n)*)  \)$",
            source,
            flags=re.MULTILINE,
        )
        if len(matches) != 1:
            errors.append(
                f"{release_path}: release runner must contain one canonical "
                f"indented {name} array"
            )
            return []
        return [
            line.strip()
            for line in matches[0].splitlines()
            if line.strip() and not line.lstrip().startswith("#")
        ]

    runner_grouped_sdk_surfaces = indented_shell_array(
        "native_amx_grouped_parity_surfaces"
    )
    runner_grouped_sdk_count_tokens = indented_shell_array(
        "native_amx_grouped_parity_test_counts"
    )
    try:
        runner_grouped_sdk_counts = tuple(
            int(token) for token in runner_grouped_sdk_count_tokens
        )
    except ValueError:
        runner_grouped_sdk_counts = ()
        errors.append(
            f"{release_path}: grouped Native AMX SDK runner counts must be integers"
        )
    runner_grouped_sdk_suites = tuple(
        zip(
            runner_grouped_sdk_surfaces,
            runner_grouped_sdk_counts,
        )
    )
    if runner_grouped_sdk_suites != canonical_grouped_sdk_suites:
        errors.append(
            f"{release_path}: grouped Native AMX SDK runner suite inventory must "
            f"equal {canonical_grouped_sdk_suites!r}; found "
            f"{runner_grouped_sdk_suites!r}"
        )

    receipt_path = repo_root / "scripts" / "write_sumeragi_v2_release_receipt.py"
    receipt_grouped_sdk_suites: object = None
    if not receipt_path.is_file() or receipt_path.is_symlink():
        errors.append(
            f"{receipt_path}: grouped Native AMX SDK receipt source must be a regular file"
        )
    else:
        try:
            receipt_tree = ast.parse(
                receipt_path.read_text(encoding="utf-8"),
                filename=str(receipt_path),
            )
        except (OSError, SyntaxError) as error:
            errors.append(
                f"{receipt_path}: grouped Native AMX SDK receipt source is invalid: {error}"
            )
        else:
            assignments = [
                node
                for node in receipt_tree.body
                if isinstance(node, ast.Assign)
                and len(node.targets) == 1
                and isinstance(node.targets[0], ast.Name)
                and node.targets[0].id == "_NATIVE_AMX_GROUPED_PARITY_SUITES"
            ]
            if len(assignments) != 1:
                errors.append(
                    f"{receipt_path}: grouped Native AMX SDK receipt suite inventory "
                    f"must be assigned exactly once; found {len(assignments)}"
                )
            else:
                try:
                    receipt_grouped_sdk_suites = ast.literal_eval(
                        assignments[0].value
                    )
                except (TypeError, ValueError) as error:
                    errors.append(
                        f"{receipt_path}: grouped Native AMX SDK receipt suite "
                        f"inventory is not a literal: {error}"
                    )
    if receipt_grouped_sdk_suites != canonical_grouped_sdk_suites:
        errors.append(
            f"{receipt_path}: grouped Native AMX SDK receipt suite inventory must "
            f"equal {canonical_grouped_sdk_suites!r}; found "
            f"{receipt_grouped_sdk_suites!r}"
        )

    grouped_harness_path = (
        repo_root / "ci" / "run_native_amx_v2_grouped_sdk_parity.sh"
    )
    harness_grouped_sdk_suites: list[tuple[str, int]] = []
    if not grouped_harness_path.is_file() or grouped_harness_path.is_symlink():
        errors.append(
            f"{grouped_harness_path}: grouped Native AMX SDK harness must be a regular file"
        )
    else:
        grouped_harness_source = grouped_harness_path.read_text(encoding="utf-8")
        runtime_case_marker = 'observed_test_count=0\ncase "$surface" in\n'
        if grouped_harness_source.count(runtime_case_marker) != 1:
            errors.append(
                f"{grouped_harness_path}: grouped Native AMX SDK harness must "
                "contain one runtime surface dispatch"
            )
        else:
            runtime_case = grouped_harness_source.split(runtime_case_marker, 1)[1]
            for surface, _expected_count in canonical_grouped_sdk_suites:
                branch_marker = f"  {surface})\n"
                if runtime_case.count(branch_marker) != 1:
                    errors.append(
                        f"{grouped_harness_path}: grouped Native AMX SDK harness "
                        f"must contain one {surface!r} branch"
                    )
                    continue
                branch = runtime_case.split(branch_marker, 1)[1].split(
                    "\n    ;;", 1
                )[0]
                expected_assignment = (
                    "    observed_test_count=$((6 + 1))"
                    if surface == "kotlin"
                    else f"    observed_test_count={_expected_count}"
                )
                if branch.splitlines().count(expected_assignment) != 1:
                    errors.append(
                        f"{grouped_harness_path}: grouped Native AMX SDK harness "
                        f"branch {surface!r} must assign one exact test count"
                    )
                    continue
                harness_grouped_sdk_suites.append((surface, _expected_count))
    if tuple(harness_grouped_sdk_suites) != canonical_grouped_sdk_suites:
        errors.append(
            f"{grouped_harness_path}: grouped Native AMX SDK harness suite inventory "
            f"must equal {canonical_grouped_sdk_suites!r}; found "
            f"{tuple(harness_grouped_sdk_suites)!r}"
        )

    canonical_sdk_diagnostics_suites = (
        ("python", 129),
        ("javascript", 88),
        ("swift", 34),
        ("kotlin", 44),
        ("java", 43),
    )
    runner_sdk_diagnostics_surfaces = indented_shell_array(
        "sumeragi_v2_sdk_diagnostics_surfaces"
    )
    runner_sdk_diagnostics_count_tokens = indented_shell_array(
        "sumeragi_v2_sdk_diagnostics_test_counts"
    )
    try:
        runner_sdk_diagnostics_counts = tuple(
            int(token) for token in runner_sdk_diagnostics_count_tokens
        )
    except ValueError:
        runner_sdk_diagnostics_counts = ()
        errors.append(
            f"{release_path}: Sumeragi SDK diagnostics runner counts must be integers"
        )
    runner_sdk_diagnostics_suites = tuple(
        zip(runner_sdk_diagnostics_surfaces, runner_sdk_diagnostics_counts)
    )
    if runner_sdk_diagnostics_suites != canonical_sdk_diagnostics_suites:
        errors.append(
            f"{release_path}: Sumeragi SDK diagnostics runner suite inventory must "
            f"equal {canonical_sdk_diagnostics_suites!r}; found "
            f"{runner_sdk_diagnostics_suites!r}"
        )
    for retired_fragment in (
        "--test-name-pattern",
        "status-javascript",
        "status-python",
    ):
        if retired_fragment in source:
            errors.append(
                f"{release_path}: Sumeragi SDK diagnostics corridor retains retired "
                f"ordinal/partial selector {retired_fragment!r}"
            )

    canonical_rust_sdk_diagnostics_tests = (
        "client::tests::get_sumeragi_status_prefers_norito_and_handles_json",
        "client::tests::get_sumeragi_status_rejects_unknown_json_fields",
        "client::tests::get_sumeragi_status_rejects_structurally_impossible_norito_and_json",
        "client::tests::get_sumeragi_status_json_requires_exact_json_media_type",
        "client::tests::get_sumeragi_diagnostics_verifies_lane_relay_envelopes",
        "client::tests::get_sumeragi_diagnostics_rejects_invalid_lane_relay_hash",
        "client::tests::get_sumeragi_diagnostics_rejects_malformed_autonomous_execution",
        "client::tests::get_sumeragi_diagnostics_rejects_duplicate_autonomous_execution_identity",
        "client::tests::get_sumeragi_diagnostics_rejects_malformed_native_amx_receipts_in_every_container",
        "client::tests::get_sumeragi_diagnostics_rejects_malformed_json_payload",
        "client::tests::get_sumeragi_diagnostics_rejects_json_payload_missing_required_fields",
        "client::tests::get_sumeragi_diagnostics_rejects_unknown_json_fields",
        "client::tests::get_sumeragi_diagnostics_rejects_zero_npos_seed",
        "client::tests::get_sumeragi_diagnostics_requires_declared_current_media_type",
    )
    runner_rust_sdk_diagnostics_tests = tuple(
        shell_array("rust_sdk_diagnostics_tests")
    )
    if runner_rust_sdk_diagnostics_tests != canonical_rust_sdk_diagnostics_tests:
        errors.append(
            f"{release_path}: Rust SDK diagnostics inventory must equal the "
            f"reviewed fourteen-test contract; found "
            f"{runner_rust_sdk_diagnostics_tests!r}"
        )
    rust_sdk_diagnostics_leg = (
        "run_corridor_leg \\\n"
        "  sumeragi-diagnostics-rust cargo-exact 14 \\\n"
        '  "cargo test --locked --offline -p iroha --lib '
        'client::tests::get_sumeragi_ -- --test-threads=1" \\\n'
        "  run_cargo test --locked --offline -p iroha --lib \\\n"
        "    client::tests::get_sumeragi_ -- --test-threads=1"
    )
    if source.count(rust_sdk_diagnostics_leg) != 1:
        errors.append(
            f"{release_path}: Rust SDK diagnostics must be one exact guarded "
            "fourteen-test corridor leg"
        )

    receipt_sdk_diagnostics_suites: object = None
    receipt_rust_sdk_diagnostics_tests: object = None
    if receipt_path.is_file() and not receipt_path.is_symlink():
        try:
            sdk_receipt_tree = ast.parse(
                receipt_path.read_text(encoding="utf-8"),
                filename=str(receipt_path),
            )
        except (OSError, SyntaxError) as error:
            errors.append(
                f"{receipt_path}: Sumeragi SDK diagnostics receipt source is "
                f"invalid: {error}"
            )
        else:
            sdk_receipt_assignments = {
                node.targets[0].id: node.value
                for node in sdk_receipt_tree.body
                if isinstance(node, ast.Assign)
                and len(node.targets) == 1
                and isinstance(node.targets[0], ast.Name)
                and node.targets[0].id
                in {
                    "_SUMERAGI_SDK_DIAGNOSTICS_SUITES",
                    "_RUST_SDK_DIAGNOSTICS_TESTS",
                }
            }
            try:
                receipt_sdk_diagnostics_suites = ast.literal_eval(
                    sdk_receipt_assignments["_SUMERAGI_SDK_DIAGNOSTICS_SUITES"]
                )
                receipt_rust_sdk_diagnostics_tests = ast.literal_eval(
                    sdk_receipt_assignments["_RUST_SDK_DIAGNOSTICS_TESTS"]
                )
            except (KeyError, TypeError, ValueError) as error:
                errors.append(
                    f"{receipt_path}: Sumeragi SDK diagnostics receipt inventories "
                    f"must be unique literals: {error}"
                )
    if receipt_sdk_diagnostics_suites != canonical_sdk_diagnostics_suites:
        errors.append(
            f"{receipt_path}: Sumeragi SDK diagnostics receipt suite inventory "
            f"must equal {canonical_sdk_diagnostics_suites!r}; found "
            f"{receipt_sdk_diagnostics_suites!r}"
        )
    if receipt_rust_sdk_diagnostics_tests != canonical_rust_sdk_diagnostics_tests:
        errors.append(
            f"{receipt_path}: Rust SDK diagnostics receipt inventory must equal "
            "the reviewed fourteen-test contract"
        )

    sdk_diagnostics_harness_path = (
        repo_root / "ci" / "run_sumeragi_v2_sdk_diagnostics.sh"
    )
    harness_sdk_diagnostics_suites: list[tuple[str, int]] = []
    if (
        not sdk_diagnostics_harness_path.is_file()
        or sdk_diagnostics_harness_path.is_symlink()
    ):
        errors.append(
            f"{sdk_diagnostics_harness_path}: Sumeragi SDK diagnostics harness "
            "must be a regular file"
        )
        sdk_diagnostics_harness_source = ""
    else:
        sdk_diagnostics_harness_source = sdk_diagnostics_harness_path.read_text(
            encoding="utf-8"
        )
        runtime_case_marker = 'observed_test_count=0\ncase "$surface" in\n'
        if sdk_diagnostics_harness_source.count(runtime_case_marker) != 1:
            errors.append(
                f"{sdk_diagnostics_harness_path}: Sumeragi SDK diagnostics harness "
                "must contain one runtime surface dispatch"
            )
        else:
            runtime_case = sdk_diagnostics_harness_source.split(
                runtime_case_marker, 1
            )[1]
            for surface, _expected_count in canonical_sdk_diagnostics_suites:
                branch_marker = f"  {surface})\n"
                if runtime_case.count(branch_marker) != 1:
                    errors.append(
                        f"{sdk_diagnostics_harness_path}: Sumeragi SDK diagnostics "
                        f"harness must contain one {surface!r} branch"
                    )
                    continue
                branch = runtime_case.split(branch_marker, 1)[1].split(
                    "\n    ;;", 1
                )[0]
                matches = re.findall(
                    r"^    observed_test_count=([0-9]+)$",
                    branch,
                    flags=re.MULTILINE,
                )
                if len(matches) != 1:
                    errors.append(
                        f"{sdk_diagnostics_harness_path}: Sumeragi SDK diagnostics "
                        f"branch {surface!r} must assign one exact test count"
                    )
                    continue
                harness_sdk_diagnostics_suites.append(
                    (surface, int(matches[0]))
                )
    if tuple(harness_sdk_diagnostics_suites) != canonical_sdk_diagnostics_suites:
        errors.append(
            f"{sdk_diagnostics_harness_path}: Sumeragi SDK diagnostics harness "
            f"suite inventory must equal {canonical_sdk_diagnostics_suites!r}; "
            f"found {tuple(harness_sdk_diagnostics_suites)!r}"
        )
    for no_skip_fragment in (
        '      assert_node_tap "$javascript_transcript" 44',
        'if tuple(totals) != (expected, 0, 0, 0):',
        'any("skipped" in line.lower() for line in lines)',
        'f"expected one exact no-skip {expected}-test pytest transcript"',
    ):
        if sdk_diagnostics_harness_source.count(no_skip_fragment) != 1:
            errors.append(
                f"{sdk_diagnostics_harness_path}: Sumeragi SDK diagnostics "
                f"no-skip contract lacks exact fragment {no_skip_fragment!r}"
            )
    if sdk_diagnostics_harness_source.count(
        '"${torii_test}::test_'
    ) != 42:
        errors.append(
            f"{sdk_diagnostics_harness_path}: Python Torii diagnostics must use "
            "exactly forty-two explicit node IDs"
        )

    js_diagnostics_test_path = (
        repo_root
        / "javascript"
        / "iroha_js"
        / "test"
        / "sumeragiDiagnosticsContract.test.js"
    )
    js_torii_test_path = (
        repo_root / "javascript" / "iroha_js" / "test" / "toriiClient.test.js"
    )
    if (
        not js_diagnostics_test_path.is_file()
        or js_diagnostics_test_path.is_symlink()
        or not js_torii_test_path.is_file()
        or js_torii_test_path.is_symlink()
    ):
        errors.append(
            "JavaScript Sumeragi SDK diagnostics sources must be regular files"
        )
    else:
        js_diagnostics_source = js_diagnostics_test_path.read_text(encoding="utf-8")
        js_torii_test_source = js_torii_test_path.read_text(encoding="utf-8")
        inventory_matches = re.findall(
            r"export const SUMERAGI_DIAGNOSTICS_CONTRACT_TESTS = Object\.freeze\(\[\n"
            r"((?:  \"[^\n]+\",\n)+)\]\);",
            js_diagnostics_source,
        )
        js_diagnostics_tests = (
            re.findall(r'^  "([^"]+)",$', inventory_matches[0], re.MULTILINE)
            if len(inventory_matches) == 1
            else []
        )
        if (
            len(js_diagnostics_tests) != 44
            or len(set(js_diagnostics_tests)) != 44
            or "typed Sumeragi endpoints reject swapped status and diagnostics payloads"
            not in js_diagnostics_tests
        ):
            errors.append(
                f"{js_diagnostics_test_path}: dedicated JavaScript Sumeragi "
                "diagnostics inventory must contain exactly 44 unique tests and "
                "the swapped-endpoint negative"
            )
        elif any(
            js_torii_test_source.count(f'test("{name}",') != 1
            for name in js_diagnostics_tests
        ):
            errors.append(
                f"{js_torii_test_path}: dedicated JavaScript Sumeragi diagnostics "
                "inventory must map one-to-one onto real test registrations"
            )
        for focus_fragment, expected_occurrences in (
            ("iroha.js.test.sumeragiDiagnosticsContract", 2),
            (
                "focused Sumeragi diagnostics test registrations must match the exact inventory",
                1,
            ),
            ('"# skipped": 0,', 1),
        ):
            focus_sources = (
                js_diagnostics_source
                + js_torii_test_source
                + sdk_diagnostics_harness_source
            )
            if focus_sources.count(focus_fragment) != expected_occurrences:
                errors.append(
                    "dedicated JavaScript Sumeragi diagnostics no-skip selector "
                    f"lacks exact fragment {focus_fragment!r}"
                )

    inventory = shell_array("required_production_liveness_tests")
    if len(inventory) != _PRODUCTION_LIVENESS_RELEASE_COUNT:
        errors.append(
            f"{release_path}: production liveness inventory must contain exactly "
            f"{_PRODUCTION_LIVENESS_RELEASE_COUNT} tests; found {len(inventory)}"
        )
    if len(set(inventory)) != len(inventory):
        duplicates = sorted(
            name for name in set(inventory) if inventory.count(name) != 1
        )
        errors.append(
            f"{release_path}: production liveness inventory repeats tests {duplicates}"
        )
    expected_count_line = (
        "readonly expected_production_liveness_test_count="
        f"{_PRODUCTION_LIVENESS_RELEASE_COUNT}"
    )
    if source.splitlines().count(expected_count_line) != 1:
        errors.append(
            f"{release_path}: production liveness source count must be sealed as "
            f"{_PRODUCTION_LIVENESS_RELEASE_COUNT}"
        )

    typed_rollover_release_fragments = (
        "readonly expected_typed_rollover_formal_mutation_count=45",
        "observed_typed_rollover_formal_mutation_count=\"$(",
        "  grep -Ec '^  \"[a-z0-9-]+\\|typed_rollover_handoff_"
        "[a-z0-9_]+_bug[.]cfg\\|(12|13)\\|\\$\\{"
        "(INVARIANT|TEMPORAL)_MARKER\\}\"$|^run_case "
        "repeated-handoff-after-restart-restore \\\\$' \\\n",
        "    scripts/formal/"
        "run_sumeragi_v2_typed_rollover_handoff_mutations.sh\n)",
        "!= expected_typed_rollover_formal_mutation_count)); then",
        'echo "[tlc] typed rollover-handoff repaired models and 45-mutant '
        'root-anchored V3 matrix passed"',
        "  scripts/formal/"
        "run_sumeragi_v2_typed_rollover_handoff_mutations.sh; then",
    )
    for fragment in typed_rollover_release_fragments:
        if source.count(fragment) != 1:
            errors.append(
                f"{release_path}: release corridor must retain the exact "
                f"45-mutation typed rollover contract fragment {fragment!r}"
            )

    multilane_focus_rows: list[tuple[str, str, str]] = []
    for array_name, leg_id, package in _PRODUCTION_MULTILANE_FOCUS_CONTRACTS:
        multilane_focus_rows.extend(
            (leg_id, package, test_name)
            for test_name in shell_array(array_name)
        )
    if len(multilane_focus_rows) != _PRODUCTION_MULTILANE_FOCUS_TEST_COUNT:
        errors.append(
            f"{release_path}: multilane G-UNIT focus inventory must contain "
            f"exactly {_PRODUCTION_MULTILANE_FOCUS_TEST_COUNT} tests; found "
            f"{len(multilane_focus_rows)}"
        )
    duplicate_multilane_focus_tests = sorted(
        {
            (package, test_name)
            for _, package, test_name in multilane_focus_rows
            if sum(
                candidate_package == package and candidate_test == test_name
                for _, candidate_package, candidate_test in multilane_focus_rows
            )
            != 1
        }
    )
    if duplicate_multilane_focus_tests:
        errors.append(
            f"{release_path}: multilane G-UNIT focus inventory repeats "
            f"crate/test pairs {duplicate_multilane_focus_tests}"
        )
    multilane_focus_inventory_rows = ["leg_id\tcrate\ttest"]
    multilane_focus_inventory_rows.extend(
        f"{leg_id}\t{package}\t{test_name}"
        for leg_id, package, test_name in multilane_focus_rows
    )
    multilane_focus_inventory_bytes = (
        "\n".join(multilane_focus_inventory_rows) + "\n"
    ).encode("utf-8")
    observed_multilane_focus_sha256 = hashlib.sha256(
        multilane_focus_inventory_bytes
    ).hexdigest()
    if (
        observed_multilane_focus_sha256
        != _PRODUCTION_MULTILANE_FOCUS_INVENTORY_SHA256
    ):
        errors.append(
            f"{release_path}: canonical G-UNIT leg/crate/test inventory SHA-256 "
            f"must be {_PRODUCTION_MULTILANE_FOCUS_INVENTORY_SHA256}; found "
            f"{observed_multilane_focus_sha256}"
        )

    expected_multilane_focus_count_line = (
        "readonly expected_multilane_focus_test_count="
        f"{_PRODUCTION_MULTILANE_FOCUS_TEST_COUNT}"
    )
    if source.splitlines().count(expected_multilane_focus_count_line) != 1:
        errors.append(
            f"{release_path}: multilane G-UNIT source count must be sealed as "
            f"{_PRODUCTION_MULTILANE_FOCUS_TEST_COUNT}"
        )
    focus_array_names = [
        array_name
        for array_name, _, _ in _PRODUCTION_MULTILANE_FOCUS_CONTRACTS
    ]
    expected_multilane_count_guard = "\n".join(
        [
            f"if (( ${{#{focus_array_names[0]}[@]}}",
            *(
                f"    + ${{#{array_name}[@]}}"
                for array_name in focus_array_names[1:]
            ),
            "    != expected_multilane_focus_test_count )); then",
        ]
    )
    if source.count(expected_multilane_count_guard) != 1:
        errors.append(
            f"{release_path}: multilane G-UNIT count guard must sum every "
            "reviewed focus array exactly once"
        )

    expected_g_unit_header = (
        "  printf '%s\\n' $'leg_id\\tcrate\\ttest' "
        '>"$corridor_g_unit_inventory"'
    )
    if source.count(expected_g_unit_header) != 1:
        errors.append(
            f"{release_path}: G-UNIT inventory must write exactly one canonical "
            "leg_id/crate/test header"
        )
    normalized_shell_continuations = re.sub(
        r"[ \t]*\\\r?\n[ \t]*", " ", source
    )
    for array_name, leg_id, package in _PRODUCTION_MULTILANE_FOCUS_CONTRACTS:
        expected_append_route = (
            f"append_g_unit_inventory {leg_id} {package} "
            f'"${{{array_name}[@]}}"'
        )
        if normalized_shell_continuations.count(expected_append_route) != 1:
            errors.append(
                f"{release_path}: G-UNIT leg {leg_id} must append the exact "
                f"{package}/{array_name} inventory once"
            )

    expected_g_unit_line_count_guard = (
        '  if [[ "$(wc -l <"$corridor_g_unit_inventory" | tr -d '
        f"""'[:space:]')" != {_PRODUCTION_MULTILANE_G_UNIT_TSV_LINE_COUNT} ]]; then"""
    )
    expected_g_unit_line_count_error = (
        '    echo "G-UNIT inventory must contain one header and exactly '
        f'{_PRODUCTION_MULTILANE_FOCUS_TEST_COUNT} focused tests" >&2'
    )
    if (
        source.count(expected_g_unit_line_count_guard) != 1
        or source.count(expected_g_unit_line_count_error) != 1
    ):
        errors.append(
            f"{release_path}: G-UNIT TSV guard must require one header plus "
            f"exactly {_PRODUCTION_MULTILANE_FOCUS_TEST_COUNT} focus rows "
            f"({_PRODUCTION_MULTILANE_G_UNIT_TSV_LINE_COUNT} total lines)"
        )

    expected_g_unit_inventory_comment = (
        f"The canonical {_PRODUCTION_MULTILANE_FOCUS_TEST_COUNT}-row TSV is"
    )
    if source.count(expected_g_unit_inventory_comment) != 1:
        errors.append(
            f"{release_path}: G-UNIT inventory comment must seal "
            f"{_PRODUCTION_MULTILANE_FOCUS_TEST_COUNT} rows"
        )
    expected_g_unit_success_fragment = (
        f"including exact {_PRODUCTION_MULTILANE_FOCUS_TEST_COUNT}/"
        f"{_PRODUCTION_MULTILANE_FOCUS_TEST_COUNT} G-UNIT,"
    )
    if source.count(expected_g_unit_success_fragment) != 1:
        errors.append(
            f"{release_path}: terminal success text must seal exact "
            f"{_PRODUCTION_MULTILANE_FOCUS_TEST_COUNT}/"
            f"{_PRODUCTION_MULTILANE_FOCUS_TEST_COUNT} G-UNIT"
        )

    if len(_PRODUCTION_LIVENESS_NEW_REGRESSIONS) != 453:
        errors.append("internal release-regression seal must contain exactly 453 names")
    for test_name in _PRODUCTION_LIVENESS_NEW_REGRESSIONS:
        occurrences = inventory.count(test_name)
        if occurrences != 1:
            errors.append(
                f"{release_path}: production ownership regression {test_name} "
                f"must be pinned exactly once; found {occurrences}"
            )

    genesis_finality_path = (
        repo_root
        / "crates"
        / "iroha_data_model"
        / "src"
        / "block"
        / "consensus_v2"
        / "finality.rs"
    )
    if not genesis_finality_path.is_file() or genesis_finality_path.is_symlink():
        errors.append(
            f"{genesis_finality_path}: genesis header-binding regression source "
            "must be a regular file"
        )
    else:
        genesis_finality_source = genesis_finality_path.read_text(encoding="utf-8")
        genesis_test = _require_rust_item(
            genesis_finality_path,
            genesis_finality_source,
            "header_binding_allows_unchanged_reproposal_but_rejects_earlier_decision_round",
            errors,
        )
        _require_rust_item_context(
            genesis_finality_path,
            genesis_test,
            (("#", "[", "cfg", "(", "test", ")", "]", "mod", "tests"),),
            "genesis header-binding release regression",
            errors,
            expected_attributes=("#[test]",),
        )
        if genesis_test is not None:
            observed_sha256 = _rust_item_token_sha256(genesis_test)
            if observed_sha256 != _GENESIS_HEADER_BINDING_TEST_SHA256:
                errors.append(
                    f"{genesis_finality_path}:{genesis_test.line}: genesis "
                    "header-binding release regression must match exact reviewed "
                    f"token digest {_GENESIS_HEADER_BINDING_TEST_SHA256}; found "
                    f"{observed_sha256}"
                )

    restart_runner_path = (
        repo_root / "integration_tests" / "tests" / "sumeragi_v2_runner.rs"
    )
    if not restart_runner_path.is_file() or restart_runner_path.is_symlink():
        errors.append(
            f"{restart_runner_path}: contention-tolerant restart regression source "
            "must be a regular file"
        )
    else:
        _loaded_path, restart_runner_source = _read_reviewed_rust_source(
            repo_root,
            restart_runner_path.relative_to(repo_root).as_posix(),
            errors,
            "contention-tolerant restart regression source",
        )
        restart_deadline_test = _require_rust_item(
            restart_runner_path,
            restart_runner_source,
            "restart_scenario_uses_a_contention_tolerant_view_zero_deadline",
            errors,
        )
        _require_rust_item_context(
            restart_runner_path,
            restart_deadline_test,
            (
                (
                    "#",
                    "[",
                    "cfg",
                    "(",
                    "test",
                    ")",
                    "]",
                    "mod",
                    "prepare_qc_split_tests",
                ),
            ),
            "contention-tolerant restart release regression",
            errors,
            expected_attributes=("#[test]",),
        )
        if restart_deadline_test is not None:
            observed_sha256 = _rust_item_token_sha256(restart_deadline_test)
            if observed_sha256 != _RESTART_VIEW_ZERO_DEADLINE_TEST_SHA256:
                errors.append(
                    f"{restart_runner_path}:{restart_deadline_test.line}: "
                    "contention-tolerant restart release regression must match "
                    "exact reviewed token digest "
                    f"{_RESTART_VIEW_ZERO_DEADLINE_TEST_SHA256}; found "
                    f"{observed_sha256}"
                )

    successor_adapter_path = (
        repo_root / "crates" / "iroha_core" / "src" / "sumeragi" / "v2.rs"
    )
    if not successor_adapter_path.is_file() or successor_adapter_path.is_symlink():
        errors.append(
            f"{successor_adapter_path}: successor parent-binding regression source "
            "must be a regular file"
        )
    else:
        _loaded_path, successor_adapter_source = _read_reviewed_rust_source(
            repo_root,
            successor_adapter_path.relative_to(repo_root).as_posix(),
            errors,
            "successor parent-binding regression source",
        )
        for test_name, expected_sha256 in _SUCCESSOR_PARENT_BINDING_TEST_SHA256.items():
            successor_test = _require_rust_item(
                successor_adapter_path,
                successor_adapter_source,
                test_name,
                errors,
            )
            expected_context = (
                ("#", "[", "cfg", "(", "test", ")", "]", "mod", "tests"),
            )
            if test_name in {
                "successor_context_requires_the_durable_cryptographic_parent",
                "authentication_rejects_valid_commitment_conflicts_without_mutating_adapter",
            }:
                expected_attributes = ("#[cfg(feature = \"bls\")]", "#[test]")
                if successor_test is not None:
                    if successor_test.brace_context != expected_context:
                        errors.append(
                            f"{successor_adapter_path}:{successor_test.line}: "
                            "cryptographic successor parent-binding regression must "
                            f"have reviewed brace context {expected_context!r}; found "
                            f"{successor_test.brace_context!r}"
                        )
                    expected_delimiters = tuple(
                        ("{", header) for header in expected_context
                    )
                    delimiter_context = tuple(
                        (opener, header)
                        for opener, _position, header in successor_test.delimiter_context
                    )
                    if delimiter_context != expected_delimiters:
                        errors.append(
                            f"{successor_adapter_path}:{successor_test.line}: "
                            "cryptographic successor parent-binding regression must "
                            "have reviewed all-delimiter context "
                            f"{expected_delimiters!r}; found {delimiter_context!r}"
                        )
                    if successor_test.ancestor_inner_attributes:
                        errors.append(
                            f"{successor_adapter_path}:{successor_test.line}: "
                            "cryptographic successor parent-binding regression may "
                            "not be suppressed by ancestor inner cfg/cfg_attr "
                            f"attributes: {successor_test.ancestor_inner_attributes!r}"
                        )
                    if successor_test.attributes != expected_attributes:
                        errors.append(
                            f"{successor_adapter_path}:{successor_test.line}: "
                            "cryptographic successor parent-binding regression must "
                            f"have exact reviewed attributes {expected_attributes!r}; "
                            f"found {successor_test.attributes!r}"
                        )
            else:
                _require_rust_item_context(
                    successor_adapter_path,
                    successor_test,
                    expected_context,
                    "successor parent-binding release regression",
                    errors,
                    expected_attributes=("#[test]",),
                )
            if test_name == "successor_context_requires_the_durable_cryptographic_parent":
                _require_rust_token_sequence(
                    successor_adapter_path,
                    successor_test,
                    """
                    let mut substituted_execution_policy = successor.clone();
                    substituted_execution_policy.execution_policy_hash =
                        Hash::new(b"substituted successor execution policy");
                    assert!(matches!(
                        VerifiedHeightContext::successor(
                            substituted_execution_policy,
                            proofs.clone(),
                            &artifact,
                            &receipt,
                            &proofs,
                        ),
                        Err(AdapterError::ParentContextMismatch)
                    ));
                    """,
                    "successor authentication must reject execution-policy "
                    "substitution against the durable parent context",
                    errors,
                )
                _require_rust_token_sequence(
                    successor_adapter_path,
                    successor_test,
                    """
                    let mut proposal_subject = subject(0x72);
                    proposal_subject.parent_block_hash = Some(parent_subject.block_hash);
                    let proposal_body = b"parent-auth-body".to_vec();
                    proposal_subject.payload_hash = Hash::new(&proposal_body);
                    let manifest = encode_payload(
                        &successor,
                        proposal_round,
                        proposal_subject,
                        &proposal_body
                    )
                    .expect("encode successor fixture payload")
                    .manifest()
                    .clone();
                    """,
                    "successor parent-certificate authentication must use a "
                    "canonical payload-bound proposal fixture",
                    errors,
                )
            elif test_name == (
                "authentication_rejects_valid_commitment_conflicts_without_mutating_adapter"
            ):
                _require_rust_token_sequence(
                    successor_adapter_path,
                    successor_test,
                    """
                    let locally_validated_payload = [0x87, 2];
                    let locally_validated_manifest = encode_payload(
                        &context,
                        round,
                        locally_validated_subject,
                        &locally_validated_payload,
                    )
                    .expect("encode locally validated payload")
                    .manifest()
                    .clone();
                    """,
                    "execution-commitment conflict authentication must bind "
                    "the locally validated canonical payload fixture",
                    errors,
                )
                _require_rust_token_sequence(
                    successor_adapter_path,
                    successor_test,
                    """
                    let proposal_body = vec![0x83, 2];
                    let proposal_manifest = encode_payload(
                        &context,
                        proposal_round,
                        proposal_subject,
                        &proposal_body
                    )
                    .expect("encode later-view proposal payload")
                    .manifest()
                    .clone();
                    """,
                    "embedded-certificate conflict authentication must bind "
                    "the later-view canonical payload fixture",
                    errors,
                )
                _require_rust_token_sequence(
                    successor_adapter_path,
                    successor_test,
                    """
                    let mut unbound_qc_b = wire::QuorumCertificate {
                        round: timeout_round,
                        proposal_round: timeout_round,
                        execution_commitment: execution_commitment(0x86),
                        ..unbound_qc_a.clone()
                    };
                    authenticate_qc(&mut unbound_qc_b, &keys);
                    """,
                    "timeout-group commitment conflicts must use a structurally "
                    "valid timeout-round certificate",
                    errors,
                )
            if successor_test is not None:
                observed_sha256 = _rust_item_token_sha256(successor_test)
                if observed_sha256 != expected_sha256:
                    errors.append(
                        f"{successor_adapter_path}:{successor_test.line}: successor "
                        "parent-binding release regression "
                        f"{test_name} must match exact reviewed token digest "
                        f"{expected_sha256}; found {observed_sha256}"
                    )

    late_lane_recovery_path = (
        repo_root
        / "crates"
        / "iroha_core"
        / "src"
        / "sumeragi"
        / "v2_lane_work.rs"
    )
    if not late_lane_recovery_path.is_file() or late_lane_recovery_path.is_symlink():
        errors.append(
            f"{late_lane_recovery_path}: late canonical lane-recovery regression "
            "source must be a regular file"
        )
    else:
        _loaded_path, late_lane_recovery_source = _read_reviewed_rust_source(
            repo_root,
            late_lane_recovery_path.relative_to(repo_root).as_posix(),
            errors,
            "late canonical lane-recovery regression source",
        )
        late_lane_recovery_test = _require_rust_item(
            late_lane_recovery_path,
            late_lane_recovery_source,
            "globally_applied_lane_body_without_certificate_remains_recoverable",
            errors,
        )
        _require_rust_item_context(
            late_lane_recovery_path,
            late_lane_recovery_test,
            (
                (
                    "#",
                    "[",
                    "cfg",
                    "(",
                    "test",
                    ")",
                    "]",
                    "pub",
                    "(",
                    "super",
                    ")",
                    "mod",
                    "tests",
                ),
            ),
            "late canonical lane-recovery release regression",
            errors,
            expected_attributes=("#[test]",),
        )
        _require_rust_token_sequence(
            late_lane_recovery_path,
            late_lane_recovery_test,
            """
            assert!(adapter.proposal_anchor_is_committed_in_state(&proposal));
            assert!(
                adapter
                    .kura
                    .read_certified_lane_block_artifact(
                        proposal.descriptor.lane_id,
                        proposal.descriptor.lane_block_height,
                    )
                    .is_none(),
                "the globally applied body must begin without lane certificate durability"
            );
            assert!(
                !adapter
                    .state
                    .certified_lane_block_session_is_applied_or_snapshot_anchored_cached(&recovered),
                "global application alone must not impersonate lane certificate application"
            );
            assert!(
                adapter.proposal_body_available(&proposal),
                "the missing certificate must remain reconstructable from the canonical body"
            );
            """,
            "late canonical lane recovery must distinguish global body application "
            "from lane-certificate durability while preserving reconstruction",
            errors,
        )
        _require_rust_token_sequence(
            late_lane_recovery_path,
            late_lane_recovery_test,
            """
            assert_eq!(
                adapter
                    .persist_anchored_sessions()
                    .expect("rehydrate the late-applied canonical ownership"),
                0,
                "no certificate exists yet to persist"
            );
            assert!(
                adapter
                    .lane_sessions
                    .proposals_without_commit_qc()
                    .iter()
                    .any(|pending| pending == &proposal),
                "rollover must rehydrate ownership which arrived after adapter construction"
            );
            let retained_prepare_qc = lane_qc_for_phase(&proposal, &keys[..3], CertPhase::Prepare);
            let retained_prepare_pops = adapter.pops_for_lane_qc(&retained_prepare_qc);
            assert_eq!(
                adapter
                    .lane_sessions
                    .insert_qc_with_pops(retained_prepare_qc.clone(), &retained_prepare_pops),
                Ok(LaneBlockSessionInsertOutcome::Inserted),
                "retain one valid PrepareQC under the active height"
            );
            assert!(
                !adapter
                    .durable_completion_matches_finality(&finality_artifact)
                    .expect("inspect late-applied lane durability"),
                "the recovered proposal and PrepareQC are not a durable completion"
            );
            assert!(
                adapter
                    .durable_lane_rollover_authority(&finality_artifact)
                    .expect("inspect incomplete decided-lane authority")
                    .is_none(),
                "the active height must retain ownership until the CommitQC and receipt are durable"
            );
            assert!(
                adapter
                    .lane_sessions
                    .qcs_for_incomplete_sessions()
                    .contains(&retained_prepare_qc),
                "the semantically equivalent retained QC must remain active-height-owned"
            );
            """,
            "late canonical lane recovery must retain incomplete certificate "
            "progress in the active predecessor and block successor authority",
            errors,
        )
        _require_rust_token_sequence(
            late_lane_recovery_path,
            late_lane_recovery_test,
            """
            let _ = adapter.drain_effects(usize::MAX);
            adapter
                .schedule_retransmission()
                .expect("schedule the first exact missing-certificate discovery round");
            let first_round = adapter.drain_effects(usize::MAX);
            assert!(
                first_round.iter().any(|effect| {
                    matches!(
                        effect,
                        V2LaneWorkEffect::PostLaneBlock {
                            message: BlockMessage::LaneBlockProposal(pending),
                            ..
                        } if pending == &proposal
                    )
                }),
                "the rehydrated proposal must become a bounded certificate request source"
            );
            adapter
                .schedule_retransmission()
                .expect("reissue exact discovery after the first round is dropped");
            assert!(
                adapter.drain_effects(usize::MAX).iter().any(|effect| {
                    matches!(
                        effect,
                        V2LaneWorkEffect::PostLaneBlock {
                            message: BlockMessage::LaneBlockProposal(pending),
                            ..
                        } if pending == &proposal
                    )
                }),
                "a dropped first discovery round must not make decided-lane recovery passive"
            );
            """,
            "late canonical lane recovery must keep one bounded exact "
            "certificate-discovery source live across a dropped round while the "
            "predecessor stays active",
            errors,
        )
        _require_rust_token_sequence(
            late_lane_recovery_path,
            late_lane_recovery_test,
            """
            let certificate = LaneBlockCertificateV1 {
                proposal: recovered.proposal.clone(),
                prepare_qc: recovered.prepare_qc.clone(),
                commit_qc: recovered.commit_qc.clone(),
            };
            assert_eq!(
                accept_lane_message_from(
                    &mut adapter,
                    BlockMessage::LaneBlockCertificate(Box::new(certificate)),
                    PeerId::new(keys[1].public_key().clone()),
                    0,
                ),
                V2LaneIngressOutcome::Inserted
            );
            assert_eq!(
                adapter
                    .persist_anchored_sessions()
                    .expect("persist recovered certificate and application receipt"),
                1
            );
            let durable = adapter
                .kura
                .read_certified_lane_block_artifact(
                    proposal.descriptor.lane_id,
                    proposal.descriptor.lane_block_height,
                )
                .expect("recovered durable certificate");
            assert_eq!(durable.proposal, recovered.proposal);
            assert_eq!(durable.prepare_qc, retained_prepare_qc);
            assert_eq!(durable.commit_qc, recovered.commit_qc);
            assert!(
                adapter
                    .kura
                    .lane_block_application_receipt_available(&proposal),
                "certificate recovery must finish the lane application boundary"
            );
            assert!(
                adapter
                    .durable_completion_matches_finality(&finality_artifact)
                    .expect("validate recovered decided-lane durability"),
                "the recovered certificate and application receipt must release the preflight"
            );
            assert!(
                adapter
                    .durable_lane_rollover_authority(&finality_artifact)
                    .expect("build recovered decided-lane rollover authority")
                    .is_some(),
                "the exact recovered certificate and receipt must release successor activation"
            );
            """,
            "late canonical lane recovery must release successor activation only "
            "after the exact certificate and application receipt are durable",
            errors,
        )
        if late_lane_recovery_test is not None:
            observed_sha256 = _rust_item_token_sha256(late_lane_recovery_test)
            if observed_sha256 != _LATE_LANE_RECOVERY_TEST_SHA256:
                errors.append(
                    f"{late_lane_recovery_path}:{late_lane_recovery_test.line}: "
                    "late canonical lane-recovery release regression must match "
                    "exact reviewed token digest "
                    f"{_LATE_LANE_RECOVERY_TEST_SHA256}; found {observed_sha256}"
                )

    close_prefix_runner_path = (
        repo_root
        / "crates"
        / "iroha_core"
        / "src"
        / "sumeragi"
        / "v2_runner_tests.rs"
    )
    if (
        not close_prefix_runner_path.is_file()
        or close_prefix_runner_path.is_symlink()
    ):
        errors.append(
            f"{close_prefix_runner_path}: closed-prefix suffix-retry regression "
            "source must be a regular file"
        )
    else:
        _loaded_path, close_prefix_runner_source = _read_reviewed_rust_source(
            repo_root,
            close_prefix_runner_path.relative_to(repo_root).as_posix(),
            errors,
            "closed-prefix suffix-retry regression source",
        )
        close_prefix_retry_test = _require_rust_item(
            close_prefix_runner_path,
            close_prefix_runner_source,
            "closed_sidecar_prefix_handoff_requeues_only_failed_suffix",
            errors,
        )
        _require_rust_item_context(
            close_prefix_runner_path,
            close_prefix_retry_test,
            (),
            "closed-prefix suffix-retry release regression",
            errors,
            expected_attributes=("#[test]",),
        )
        if close_prefix_retry_test is not None:
            observed_sha256 = _rust_item_token_sha256(close_prefix_retry_test)
            if observed_sha256 != _CLOSED_SIDECAR_PREFIX_HANDOFF_TEST_SHA256:
                errors.append(
                    f"{close_prefix_runner_path}:{close_prefix_retry_test.line}: "
                    "closed-prefix suffix-retry release regression must match "
                    "exact reviewed token digest "
                    f"{_CLOSED_SIDECAR_PREFIX_HANDOFF_TEST_SHA256}; found "
                    f"{observed_sha256}"
                )

    modules = shell_array("production_liveness_modules")
    if modules != list(_PRODUCTION_LIVENESS_RELEASE_MODULES):
        errors.append(
            f"{release_path}: production liveness modules must equal the reviewed "
            f"ordered 44-module inventory; found {modules}"
        )
    inventory_rows = ["module\ttest"]
    inventory_has_exact_modules = True
    for test_name in inventory:
        matching_modules = [
            module for module in modules if test_name.startswith(f"{module}::")
        ]
        if len(matching_modules) != 1:
            inventory_has_exact_modules = False
            errors.append(
                f"{release_path}: production test {test_name} must map to exactly "
                f"one reviewed module; found {matching_modules}"
            )
            continue
        inventory_rows.append(f"{matching_modules[0]}\t{test_name}")
    if inventory_has_exact_modules:
        inventory_bytes = ("\n".join(inventory_rows) + "\n").encode("utf-8")
        observed_inventory_sha256 = hashlib.sha256(inventory_bytes).hexdigest()
        if observed_inventory_sha256 != _PRODUCTION_LIVENESS_RELEASE_INVENTORY_SHA256:
            errors.append(
                f"{release_path}: canonical module/test inventory SHA-256 must be "
                f"{_PRODUCTION_LIVENESS_RELEASE_INVENTORY_SHA256}; found "
                f"{observed_inventory_sha256}"
            )
    leg_ids = shell_array("production_liveness_leg_ids")
    expected_leg_ids = [
        leg_id for leg_id, _, _ in _PRODUCTION_LIVENESS_RELEASE_MODULE_CONTRACTS
    ]
    if leg_ids != expected_leg_ids or len(set(leg_ids)) != len(leg_ids):
        errors.append(
            f"{release_path}: production module leg IDs must equal the reviewed "
            f"44-entry inventory; found {leg_ids}"
        )
    for _, module, expected_count in _PRODUCTION_LIVENESS_RELEASE_MODULE_CONTRACTS:
        observed_count = sum(
            test_name.startswith(f"{module}::") for test_name in inventory
        )
        if observed_count != expected_count:
            errors.append(
                f"{release_path}: production module {module} must contain exactly "
                f"{expected_count} named tests; found {observed_count}"
            )
    expected_corridor_leg_count_line = (
        "  readonly expected_corridor_leg_count="
        f"{_PRODUCTION_LIVENESS_RELEASE_CORRIDOR_LEG_COUNT}"
    )
    if source.splitlines().count(expected_corridor_leg_count_line) != 1:
        errors.append(
            f"{release_path}: complete pre-network release corridor must remain "
            "sealed at "
            f"{_PRODUCTION_LIVENESS_RELEASE_CORRIDOR_LEG_COUNT} legs"
        )

    expected_p2p_list = (
        'production_p2p_unit_list="$(run_cargo test --locked --offline -p iroha_p2p '
        '--lib -- --list)"'
    )
    expected_p2p_ignored_list = (
        'production_p2p_ignored_unit_list="$(\n'
        '  run_cargo test --locked --offline -p iroha_p2p --lib -- --list --ignored\n'
        ')"'
    )
    if source.count(expected_p2p_list) != 1 or source.count(
        expected_p2p_ignored_list
    ) != 1:
        errors.append(
            f"{release_path}: reviewed P2P corridor must use exact default-feature "
            "test discovery"
        )
    expected_irohad_list = (
        'production_irohad_unit_list="$(\n'
        '  run_cargo test --locked --offline -p irohad --lib '
        '--features test-network-message-control -- --list\n'
        ')"'
    )
    expected_irohad_ignored_list = (
        'production_irohad_ignored_unit_list="$(\n'
        '  run_cargo test --locked --offline -p irohad --lib '
        '--features test-network-message-control -- --list --ignored\n'
        ')"'
    )
    if source.count(expected_irohad_list) != 1 or source.count(
        expected_irohad_ignored_list
    ) != 1:
        errors.append(
            f"{release_path}: irohad route-control discovery must use the exact "
            "test-network-message-control feature"
        )
    expected_config_list = (
        'production_config_unit_list="$(run_cargo test --locked --offline -p iroha_config '
        '--lib -- --list)"'
    )
    expected_config_ignored_list = (
        'production_config_ignored_unit_list="$(\n'
        '  run_cargo test --locked --offline -p iroha_config --lib -- --list --ignored\n'
        ')"'
    )
    if source.count(expected_config_list) != 1 or source.count(
        expected_config_ignored_list
    ) != 1:
        errors.append(
            f"{release_path}: exact-output configuration discovery must use the "
            "exact iroha_config library test surface"
        )
    config_inventory_route = (
        '  elif [[ "$required_test" == parameters::* ]]; then\n'
        '    required_unit_list="$production_config_unit_list"\n'
        '    required_ignored_unit_list="$production_config_ignored_unit_list"'
    )
    config_module_route = (
        '  elif [[ "$module" == parameters::* ]]; then\n'
        '    module_command="cargo test --locked --offline -p iroha_config --lib '
        '${module} -- --test-threads=1"'
    )
    if source.count(config_inventory_route) != 1 or source.count(
        config_module_route
    ) != 1:
        errors.append(
            f"{release_path}: exact-output configuration tests must route through "
            "the iroha_config library corridor"
        )

    expected_data_model_modules = [
        "block::consensus_v2::finality::tests",
        "block::consensus_v2::tests",
    ]
    if shell_array("production_data_model_modules") != expected_data_model_modules:
        errors.append(
            f"{release_path}: production data-model routing must name the exact "
            "finality and context-identity modules"
        )
    expected_data_model_list = (
        'production_data_model_unit_list="$(run_cargo test --locked --offline '
        '-p iroha_data_model --lib -- --list)"'
    )
    expected_data_model_ignored_list = (
        'production_data_model_ignored_unit_list="$(\n'
        '  run_cargo test --locked --offline -p iroha_data_model --lib -- --list --ignored\n'
        ')"'
    )
    if source.count(expected_data_model_list) != 1 or source.count(
        expected_data_model_ignored_list
    ) != 1:
        errors.append(
            f"{release_path}: production data-model modules must use exact non-ignored "
            "iroha_data_model library discovery"
        )
    for fragment in (
        'if is_production_data_model_module "$required_test_module"; then',
        'elif is_production_data_model_module "$module"; then',
        'module_command="cargo test --locked --offline -p iroha_data_model --lib '
        '${module} -- --test-threads=1"',
        'run_cargo test --locked --offline -p iroha_data_model --lib "$module" '
        '-- --test-threads=1',
    ):
        if source.count(fragment) != 1:
            errors.append(
                f"{release_path}: production data-model discovery/execution routing "
                f"must contain exactly {fragment!r}"
            )

    source_sealed_commands = (
        (
            "source-sealed-workspace-build",
            "${IROHA_RELEASE_CARGO_BIN} build -j1 --locked --offline --workspace",
            "run_cargo build --locked --offline --workspace",
        ),
        (
            "source-sealed-workspace-tests",
            "${IROHA_RELEASE_CARGO_BIN} test -j1 --locked --offline --workspace",
            "run_cargo test --locked --offline --workspace",
        ),
        (
            "source-sealed-workspace-clippy",
            "${IROHA_RELEASE_CARGO_BIN} clippy -j1 --locked --offline --workspace "
            "--all-targets -- -D warnings",
            "run_cargo clippy --locked --offline --workspace --all-targets "
            "-- -D warnings",
        ),
        (
            "source-sealed-workspace-format",
            "${IROHA_RELEASE_CARGO_BIN} fmt --all -- --check",
            "run_cargo fmt --all -- --check",
        ),
        (
            "source-sealed-legacy-codec-guard",
            "bash scripts/check_no_legacy_codec.sh",
            "bash scripts/check_no_legacy_codec.sh",
        ),
    )
    for leg_id, command, execution_command in source_sealed_commands:
        expected = (
            "  run_corridor_leg \\\n"
            f"    {leg_id} command 0 \\\n"
            f'    "{command}" \\\n'
            f"    {execution_command}"
        )
        if source.count(expected) != 1:
            errors.append(
                f"{release_path}: source-sealed command-success leg {leg_id} "
                f"must execute exactly {command!r}"
            )

    scaling_release_fragments = (
        "multilane_scaling_contract_files=(\n"
        "  scripts/tests/validate_multilane_scaling_evidence_test.py\n"
        "  scripts/tests/run_multilane_scaling_gate_test.py\n"
        ")",
        "preflight-multilane-scaling pytest 53 \\\n"
        '  "PYTHONDONTWRITEBYTECODE=1 PYTHONHASHSEED=0 python3 -m pytest '
        '-q -p no:cacheprovider ${multilane_scaling_contract_files[*]}"',
        'scripts/nexus/validate_multilane_scaling_evidence.py \\\n'
        '    "$IROHA_RELEASE_SCALING_EVIDENCE_MANIFEST" \\\n'
        '    --report "$scaling_preflight_report" \\\n'
        '    --expected-source-revision "$release_head_commit" \\\n'
        '    --expected-workspace-source-sha256 "$release_source_manifest_sha256"',
        '--expected-validator-sha256 "$(\n'
        '      sha256_file scripts/nexus/validate_multilane_scaling_evidence.py\n'
        '    )" \\\n'
        '    --expected-trial-harness-sha256 \\\n'
        '      "$IROHA_RELEASE_SCALING_TRIAL_HARNESS_SHA256" \\\n'
        '    --expected-configuration-sha256 \\\n'
        '      "$IROHA_RELEASE_SCALING_CONFIGURATION_SHA256" \\\n'
        '    --expected-irohad-sha256 "$IROHA_RELEASE_SCALING_IROHAD_SHA256" \\\n'
        '    --expected-iroha-cli-sha256 "$IROHA_RELEASE_SCALING_IROHA_CLI_SHA256" \\\n'
        '    --expected-repository-root "$repo_root" \\\n'
        '    --quiet',
        'IROHA_RELEASE_SCALING_CONFIGURATION_SHA256="$IROHA_RELEASE_SCALING_CONFIGURATION_SHA256" \\\n'
        '    IROHA_RELEASE_SCALING_EVIDENCE_MANIFEST="$release_scaling_evidence_manifest" \\\n'
        '    IROHA_RELEASE_SCALING_IROHAD_SHA256="$IROHA_RELEASE_SCALING_IROHAD_SHA256" \\\n'
        '    IROHA_RELEASE_SCALING_IROHA_CLI_SHA256="$IROHA_RELEASE_SCALING_IROHA_CLI_SHA256" \\\n'
        '    IROHA_RELEASE_SCALING_TRIAL_HARNESS_SHA256="$IROHA_RELEASE_SCALING_TRIAL_HARNESS_SHA256"',
        '--g4p-completion "$multilane_four_peer_completion_path" \\\n'
        '      --g12-seed-completion "$nexus_cross_completion_path" \\\n'
        '      --g12-fault-soak-completion "$nexus_cross_soak_completion_path" \\\n'
        '      --scaling-evidence-manifest "$release_scaling_evidence_manifest" \\\n'
        '      --sdk-dependency-archive "$release_sdk_archive" \\\n'
        '      --sdk-dependency-input-inventory "$release_sdk_inventory" \\\n'
        '      --sdk-dependency-final-work-inventory \\\n'
        '        "$release_sdk_work_final_inventory" \\\n'
        '      --runtime-tool-probe-manifest \\\n'
        '        "$release_runtime_tool_probe_manifest" \\\n'
        '      --runtime-tool-probe-result "$release_runtime_tool_probe_result" \\\n'
        '      --expected-scaling-trial-harness-sha256 \\\n'
        '        "$IROHA_RELEASE_SCALING_TRIAL_HARNESS_SHA256" \\\n'
        '      --expected-scaling-configuration-sha256 \\\n'
        '        "$IROHA_RELEASE_SCALING_CONFIGURATION_SHA256" \\\n'
        '      --expected-scaling-irohad-sha256 "$IROHA_RELEASE_SCALING_IROHAD_SHA256" \\\n'
        '      --expected-scaling-iroha-cli-sha256 "$IROHA_RELEASE_SCALING_IROHA_CLI_SHA256"',
    )
    for fragment in scaling_release_fragments:
        if source.count(fragment) != 1:
            errors.append(
                f"{release_path}: source-bound G-4P/G-12P/G-SCALE receipt corridor "
                f"must contain exactly {fragment!r}"
            )
    scaling_environment = {
        "IROHA_RELEASE_SCALING_CONFIGURATION_SHA256",
        "IROHA_RELEASE_SCALING_EVIDENCE_MANIFEST",
        "IROHA_RELEASE_SCALING_IROHAD_SHA256",
        "IROHA_RELEASE_SCALING_IROHA_CLI_SHA256",
        "IROHA_RELEASE_SCALING_TRIAL_HARNESS_SHA256",
    }
    formal_replay_environment = {
        "IROHA_RELEASE_FORMAL_REPLAY_SOURCE_RECEIPT",
        "IROHA_RELEASE_FORMAL_REPLAY_RELEASE_ROOT",
        "IROHA_RELEASE_FORMAL_REPLAY_SIGNATURE_SHA256",
        "IROHA_RELEASE_FORMAL_REPLAY_SIGNER_PRINCIPAL",
    }
    environment_contracts = (
        (
            repo_root / "scripts" / "bootstrap_sumeragi_v2_release.py",
            "_RUNNER_ENV_ALLOWLIST",
        ),
        (
            repo_root / "scripts" / "validate_sumeragi_v2_release_bootstrap.py",
            "_RUNNER_EXTRA_ENV",
        ),
        (
            repo_root / "scripts" / "write_sumeragi_v2_release_receipt.py",
            "_BOOTSTRAP_RUNNER_ENV_ALLOWLIST",
        ),
    )
    for contract_path, assignment_name in environment_contracts:
        if not contract_path.is_file() or contract_path.is_symlink():
            errors.append(
                f"{contract_path}: authenticated release environment contract "
                "must be a regular file"
            )
            continue
        contract_source = contract_path.read_text(encoding="utf-8")
        try:
            contract_tree = ast.parse(contract_source, filename=str(contract_path))
        except SyntaxError as error:
            errors.append(
                f"{contract_path}: authenticated release environment contract "
                f"is invalid Python: {error}"
            )
            continue
        assignments = [
            statement
            for statement in contract_tree.body
            if isinstance(statement, ast.Assign)
            and len(statement.targets) == 1
            and isinstance(statement.targets[0], ast.Name)
            and statement.targets[0].id == assignment_name
        ]
        if len(assignments) != 1:
            errors.append(
                f"{contract_path}: authenticated release environment must define "
                f"exactly one {assignment_name}"
            )
            continue
        try:
            allowlist = ast.literal_eval(assignments[0].value)
        except (TypeError, ValueError, SyntaxError):
            errors.append(
                f"{contract_path}: {assignment_name} must be a literal set"
            )
            continue
        admitted_scaling = {
            value
            for value in allowlist
            if isinstance(value, str) and value.startswith("IROHA_RELEASE_SCALING_")
        }
        if admitted_scaling != scaling_environment:
            errors.append(
                f"{contract_path}: authenticated release environment must admit "
                "exactly the five source-bound G-SCALE trust inputs"
            )
        admitted_formal_replay = {
            value
            for value in allowlist
            if isinstance(value, str)
            and value.startswith("IROHA_RELEASE_FORMAL_REPLAY_")
        }
        if admitted_formal_replay != formal_replay_environment:
            errors.append(
                f"{contract_path}: authenticated release environment must admit "
                "exactly the four signed formal replay inputs"
            )

    for option in (
        "--formal-replay-source-receipt",
        "--formal-replay-release-root",
        "--expected-formal-replay-signature-sha256",
        "--formal-replay-principal",
    ):
        if source.count(option) != 1:
            errors.append(
                f"{release_path}: terminal receipt publication must carry {option} "
                "exactly once"
            )
    for fragment, description in (
        (
            'IROHA_RELEASE_FORMAL_REPLAY_SOURCE_RECEIPT="$release_formal_replay_source_receipt" \\\n',
            "the canonical source receipt into the sealed child",
        ),
        (
            'IROHA_RELEASE_FORMAL_REPLAY_RELEASE_ROOT="$release_formal_replay_release_root" \\\n',
            "the finalized bundle root into the sealed child",
        ),
        (
            'IROHA_RELEASE_FORMAL_REPLAY_SIGNATURE_SHA256="$IROHA_RELEASE_FORMAL_REPLAY_SIGNATURE_SHA256" \\\n',
            "the detached signature digest into the sealed child",
        ),
        (
            'IROHA_RELEASE_FORMAL_REPLAY_SIGNER_PRINCIPAL="$IROHA_RELEASE_FORMAL_REPLAY_SIGNER_PRINCIPAL" \\\n',
            "the signer principal into the sealed child",
        ),
    ):
        if source.count(fragment) != 1:
            errors.append(
                f"{release_path}: production release must propagate {description} "
                "exactly once"
            )

    receipt_path = repo_root / "scripts" / "write_sumeragi_v2_release_receipt.py"
    if not receipt_path.is_file() or receipt_path.is_symlink():
        errors.append(f"{receipt_path}: release receipt writer must be a regular file")
    else:
        receipt_source = receipt_path.read_text(encoding="utf-8")
        if receipt_source.count(
            "bootstrap runner signed formal replay inputs are not the receipt inputs"
        ) != 1:
            errors.append(
                f"{receipt_path}: aggregate receipt must bind the signed formal "
                "replay inputs to the authenticated bootstrap environment"
            )
        try:
            receipt_tree = ast.parse(receipt_source, filename=str(receipt_path))
        except SyntaxError as error:
            errors.append(f"{receipt_path}: release receipt writer is invalid Python: {error}")
        else:
            assignments: dict[str, list[Any]] = {
                "_RELEASE_RECEIPT_COMPONENT_FILES": [],
                "_RELEASE_RECEIPT_COMPONENT_SHA256": [],
                "_PRODUCTION_TEST_COUNT": [],
                "_PRODUCTION_MODULES": [],
                "_DATA_MODEL_PRODUCTION_MODULES": [],
            }
            for statement in receipt_tree.body:
                if not isinstance(statement, ast.Assign) or len(statement.targets) != 1:
                    continue
                target = statement.targets[0]
                if not isinstance(target, ast.Name) or target.id not in assignments:
                    continue
                try:
                    assignments[target.id].append(ast.literal_eval(statement.value))
                except (TypeError, ValueError, SyntaxError):
                    assignments[target.id].append(None)
            if assignments["_PRODUCTION_TEST_COUNT"] != [
                _PRODUCTION_LIVENESS_RELEASE_COUNT
            ]:
                errors.append(
                    f"{receipt_path}: production test count must equal the exact shell "
                    f"inventory count {_PRODUCTION_LIVENESS_RELEASE_COUNT}"
                )
            if assignments["_PRODUCTION_MODULES"] != [
                _PRODUCTION_LIVENESS_RELEASE_MODULE_CONTRACTS
            ]:
                errors.append(
                    f"{receipt_path}: production module receipt tuple must equal the "
                    "exact shell leg/module/count inventory"
                )
            if assignments["_DATA_MODEL_PRODUCTION_MODULES"] != [
                tuple(expected_data_model_modules)
            ]:
                errors.append(
                    f"{receipt_path}: production data-model receipt routing must "
                    "equal the exact shell data-model module inventory"
                )
            expected_receipt_components = (
                "write_sumeragi_v2_release_receipt_formal_artifacts.py",
                "write_sumeragi_v2_release_receipt_corridor_log.py",
                "write_sumeragi_v2_release_receipt_gate_evidence.py",
                "write_sumeragi_v2_release_receipt_publication.py",
            )
            if assignments["_RELEASE_RECEIPT_COMPONENT_FILES"] != [
                expected_receipt_components
            ]:
                errors.append(
                    f"{receipt_path}: release receipt component manifest must equal "
                    f"{expected_receipt_components!r}"
                )
            expected_receipt_component_sha256 = {
                "write_sumeragi_v2_release_receipt_formal_artifacts.py": (
                    "2e997ee27e45fdf6651cd1e94689e08d348078e688ab34862d8d6396c6887ba5"
                ),
                "write_sumeragi_v2_release_receipt_corridor_log.py": (
                    "5de112cad5f1eef2ebeb0225c854e69183aab977262733c226000179861728d7"
                ),
                "write_sumeragi_v2_release_receipt_gate_evidence.py": (
                    "0d89b39300b4d1b83e28623a75bcabdf31574451dfe68d8f1b67a49afd1dc440"
                ),
                "write_sumeragi_v2_release_receipt_publication.py": (
                    "a74465a49f847a03ce4c7b17997f3434b8baf3f006c78d6e535854826848232d"
                ),
            }
            if assignments["_RELEASE_RECEIPT_COMPONENT_SHA256"] != [
                expected_receipt_component_sha256
            ]:
                errors.append(
                    f"{receipt_path}: release receipt component digests must equal "
                    f"{expected_receipt_component_sha256!r}"
                )
            expected_component_symbols = {
                "write_sumeragi_v2_release_receipt_formal_artifacts.py": (
                    "_validate_multilane_apalache_evidence",
                    "_validate_formal_snapshot_replays",
                    "_formal_artifacts",
                    "_formal_replay_release",
                ),
                "write_sumeragi_v2_release_receipt_corridor_log.py": (
                    "_receipt_validation_invocation_value_sha256",
                    "_receipt_validation_invocation_binding",
                    "_cargo_cache_relative_path",
                    "_cargo_cache_final_relative_path",
                    "_cargo_cache_octal_mode",
                    "_cargo_cache_integer",
                    "_cargo_cache_unchanged",
                    "_cargo_cache_names",
                    "_cargo_cache_stat",
                    "_cargo_cache_open_regular",
                    "_cargo_cache_tree",
                    "_validate_cargo_cache_input",
                    "_sdk_suite_source_manifest",
                    "_test_count_from_log",
                    "_prebuilt_artifact_root",
                    "_require_pruned_private_root",
                    "_prebuilt_release_roots",
                    "_prebuilt_directory",
                    "_publish_receipt_validation_ack",
                    "_receipt_validation_ack_arguments",
                    "_receipt_validation_ack",
                    "_owned_unlink_name",
                    "_corridor_legs",
                ),
                "write_sumeragi_v2_release_receipt_gate_evidence.py": (
                    "_canonical_production_tests",
                    "_canonical_g_unit_rows",
                    "_g_unit_leg_command",
                    "_production_module_command",
                    "_load_identity",
                    "_load_tsv",
                    "_require_fields",
                    "_artifact",
                    "_tlaps_resource_int",
                    "_tlaps_resource_float",
                    "_tlaps_resource_timestamp",
                    "_validate_tlaps_resource_evidence",
                    "_prebuilt_directory_inventory",
                    "_prebuilt_version_transcripts",
                    "_prebuilt_binary_bundle",
                    "_corridor_artifacts",
                    "_seed_run_logs",
                    "_seed_localnet_manifests",
                    "_scan_scaling_bundle",
                    "_capture_scaling_bundle",
                    "_load_scaling_json",
                    "_scaling_ref_path",
                    "_path_contract_artifact",
                    "_sdk_relative_path",
                    "_sdk_inventory_records",
                    "_sdk_source_inventory",
                    "_sdk_source_path",
                    "_sdk_project_source_records",
                    "_sdk_validate_private_source_manifest",
                    "_sdk_binding_contract",
                    "_sdk_validate_control_files",
                    "_sdk_validate_tar",
                    "_sdk_public_archive",
                    "_validate_sdk_dependency_evidence",
                    "_validate_scaling_evidence",
                    "_read_g12_snapshot",
                    "_decode_g12_tsv",
                    "_g12_completion_fields",
                    "_validate_g12_log",
                    "_require_g12_directory_inventory",
                    "_validate_g4p_log",
                    "_validate_g4p_evidence",
                    "_validate_g12_evidence",
                    "_runtime_tool_probe_evidence",
                ),
                "write_sumeragi_v2_release_receipt_publication.py": (
                    "_validate_framework_python_input_records",
                    "_validate_framework_python_macho_closure",
                    "_validate_framework_python_relocation_evidence",
                    "_require_pruned_build_roots",
                    "build_receipt",
                    "_iter_artifact_records",
                    "_capture_path_contract",
                    "_snapshot_receipt_inputs",
                    "_capture_directory_contract",
                    "_revalidate_receipt_inputs",
                    "_fsync_receipt_inputs",
                    "_existing_receipt_contract",
                    "_complete_write",
                    "_publish_terminal_receipt",
                    "main",
                ),
            }
            expected_parent_component_symbols = frozenset(
                symbol
                for symbols in expected_component_symbols.values()
                for symbol in symbols
            )
            parent_component_symbols = tuple(
                statement.name
                for statement in receipt_tree.body
                if isinstance(statement, (ast.FunctionDef, ast.AsyncFunctionDef))
                and statement.name in expected_parent_component_symbols
            )
            if parent_component_symbols:
                errors.append(
                    f"{receipt_path}: formal receipt functions must remain isolated "
                    "in the declared component"
                )
            for component_name in expected_receipt_components:
                component_path = receipt_path.with_name(component_name)
                if component_path.is_symlink() or not component_path.is_file():
                    errors.append(
                        f"{component_path}: release receipt component must be a "
                        "regular non-symlink file"
                    )
                    continue
                try:
                    component_source = component_path.read_text(encoding="utf-8")
                    component_tree = ast.parse(
                        component_source,
                        filename=str(component_path),
                    )
                except (OSError, UnicodeDecodeError, SyntaxError) as error:
                    errors.append(
                        f"{component_path}: release receipt component is invalid: "
                        f"{error}"
                    )
                    continue
                component_symbols = tuple(
                    statement.name
                    for statement in component_tree.body
                    if isinstance(statement, (ast.FunctionDef, ast.AsyncFunctionDef))
                )
                if component_symbols != expected_component_symbols[component_name]:
                    errors.append(
                        f"{component_path}: release receipt component symbols must "
                        f"equal {expected_component_symbols[component_name]!r}"
                    )
                component_sha256 = hashlib.sha256(component_path.read_bytes()).hexdigest()
                if component_sha256 != expected_receipt_component_sha256[component_name]:
                    errors.append(
                        f"{component_path}: release receipt component SHA-256 must "
                        f"equal {expected_receipt_component_sha256[component_name]}"
                    )
            gate_evidence_path = receipt_path.with_name(
                "write_sumeragi_v2_release_receipt_gate_evidence.py"
            )
            formal_component_path = receipt_path.with_name(
                "write_sumeragi_v2_release_receipt_formal_artifacts.py"
            )
            formal_component_source = (
                formal_component_path.read_text(encoding="utf-8")
                if formal_component_path.is_file()
                and not formal_component_path.is_symlink()
                else ""
            )
            for fragment, description in (
                (
                    '"namespace": "iroha-sumeragi-v2-replay-receipt-v1",\n',
                    "the V1 replay SSHSIG namespace",
                ),
                (
                    '"source_artifacts": [\n',
                    "the complete replay output inventory",
                ),
                (
                    '"receipt",\n            release_root_path / "receipt.json",\n            0o400,\n',
                    "the immutable finalized receipt",
                ),
                (
                    '"tlapm-projection",\n',
                    "the authenticated TLAPM projection root inventory",
                ),
                (
                    '"tlapm_folds",\n            projection_root / "Folds.tla",\n'
                    '            0o444,\n',
                    "the immutable finalized Folds projection",
                ),
                (
                    '"tlapm_functions",\n            projection_root / "Functions.tla",\n'
                    '            0o444,\n',
                    "the immutable finalized Functions projection",
                ),
                (
                    'if status != 0 or stdout != expected_stdout or stderr:\n',
                    "the independent verifier success gate",
                ),
                (
                    'release_root_contract = _capture_directory_contract(\n',
                    "the pre-verification release-root identity",
                ),
                (
                    'source_root_contract = _capture_directory_contract(\n',
                    "the pre-verification source-root identity",
                ),
                (
                    'watched_contracts=(\n            *snapshots.values(),\n            *source_artifacts,\n            *verifier_dependencies,\n        ),\n',
                    "the exact verifier input closure",
                ),
                (
                    '"formal replay release directories changed during verification"\n',
                    "the post-verification directory identity gate",
                ),
                (
                    '"formal replay TLAPM projection after verification",\n',
                    "the post-verification projection identity gate",
                ),
            ):
                if formal_component_source.count(fragment) != 1:
                    errors.append(
                        f"{formal_component_path}: aggregate formal replay evidence "
                        f"must retain {description} exactly once"
                    )
            aggregate_replay_markers = (
                '    specs = (\n        (\n            "source_receipt",\n',
                "snapshots: dict[str, EvidenceSnapshot] = {}\n",
                "source_artifacts: list[EvidenceSnapshot] = []\n",
                "status, stdout, stderr = _run_bounded_python_validator(\n",
                '"formal replay release directories changed during verification"\n',
                "def full_record(snapshot: EvidenceSnapshot) -> dict[str, Any]:\n",
            )
            aggregate_replay_source = formal_component_source[
                formal_component_source.find("def _formal_replay_release(") :
            ]
            aggregate_replay_positions = tuple(
                aggregate_replay_source.find(marker)
                for marker in aggregate_replay_markers
            )
            if -1 in aggregate_replay_positions or aggregate_replay_positions != tuple(
                sorted(aggregate_replay_positions)
            ):
                errors.append(
                    f"{formal_component_path}: aggregate formal replay evidence must "
                    "snapshot its complete bundle before the independent verifier and "
                    "revalidate it before inventory publication"
                )
            gate_evidence_source = (
                gate_evidence_path.read_text(encoding="utf-8")
                if gate_evidence_path.is_file() and not gate_evidence_path.is_symlink()
                else ""
            )
            if gate_evidence_source.splitlines().count(
                '                    "state::",'
            ) != 1:
                errors.append(
                    f"{gate_evidence_path}: canonical receipt inventory parser must "
                    "admit the exact state module namespace"
                )
            expected_receipt_route = (
                "if module in _DATA_MODEL_PRODUCTION_MODULES:\n"
                "        return (\n"
                '            "cargo test --locked --offline -p iroha_data_model --lib "\n'
                '            f"{module} -- --test-threads=1"\n'
                "        )"
            )
            if gate_evidence_source.count(expected_receipt_route) != 1:
                errors.append(
                    f"{gate_evidence_path}: production data-model receipt legs must execute "
                    "against the iroha_data_model library"
                )

    bootstrap_path = repo_root / "scripts" / "bootstrap_sumeragi_v2_release.py"
    expected_bootstrap_components = (
        "bootstrap_sumeragi_v2_release_receipt_replay.py",
    )
    expected_bootstrap_component_sha256 = {
        "bootstrap_sumeragi_v2_release_receipt_replay.py": (
            "f5593c473235d24df71ed42ca3ab74f7a8421aae6601aebb148c7e0b6e4aeab0"
        ),
    }
    expected_bootstrap_component_symbols = {
        "bootstrap_sumeragi_v2_release_receipt_replay.py": (
            "_framework_python_input_records",
            "_validate_framework_python_macho_closure",
            "_framework_python_marker_record",
            "_validate_terminal_release_evidence",
            "_retained_release_layout",
            "_receipt_validation_failure",
            "_remove_completed_runner_log",
            "_prune_receipt_validation_failure",
            "_validate_terminal_receipt",
            "_fsync_file_snapshot",
            "_validate_retained_source",
            "_receipt_artifact_path",
            "_receipt_nested_artifact_path",
            "_receipt_scaling_manifest_path",
            "_run_protected_receipt_validator",
            "_validate_command_record",
            "_validate_sanitized_operation",
            "_validate_raw_commit",
            "_validate_legacy_identity_evidence",
            "_validate_identity_evidence",
            "_validate_private_identity_provenance",
            "_artifact_record",
            "_load_release_approval_contract",
            "_approval_duration_values",
            "_approval_expectations",
            "_load_bound_release_approvals",
            "_approval_archive_record",
            "_replay_release_approval_evidence",
        ),
    }
    if bootstrap_path.is_symlink() or not bootstrap_path.is_file():
        errors.append(
            f"{bootstrap_path}: release bootstrap must be a regular non-symlink file"
        )
    else:
        try:
            bootstrap_source = bootstrap_path.read_text(encoding="utf-8")
            bootstrap_tree = ast.parse(bootstrap_source, filename=str(bootstrap_path))
        except (OSError, UnicodeDecodeError, SyntaxError) as error:
            errors.append(f"{bootstrap_path}: release bootstrap is invalid: {error}")
        else:
            bootstrap_assignments: dict[str, list[Any]] = {
                "_BOOTSTRAP_COMPONENT_FILES": [],
                "_BOOTSTRAP_COMPONENT_SHA256": [],
                "_TERMINAL_EVIDENCE_KEYS": [],
                "_VALIDATOR_OPTION_ORDER": [],
                "_VALIDATOR_PATH_OPTIONS": [],
            }
            for statement in bootstrap_tree.body:
                if not isinstance(statement, ast.Assign) or len(statement.targets) != 1:
                    continue
                target = statement.targets[0]
                if (
                    not isinstance(target, ast.Name)
                    or target.id not in bootstrap_assignments
                ):
                    continue
                try:
                    if (
                        target.id == "_VALIDATOR_PATH_OPTIONS"
                        and isinstance(statement.value, ast.Call)
                        and isinstance(statement.value.func, ast.Name)
                        and statement.value.func.id == "frozenset"
                        and len(statement.value.args) == 1
                        and not statement.value.keywords
                    ):
                        parsed_value = frozenset(
                            ast.literal_eval(statement.value.args[0])
                        )
                    else:
                        parsed_value = ast.literal_eval(statement.value)
                    bootstrap_assignments[target.id].append(parsed_value)
                except (TypeError, ValueError, SyntaxError):
                    bootstrap_assignments[target.id].append(None)
            if bootstrap_assignments["_BOOTSTRAP_COMPONENT_FILES"] != [
                expected_bootstrap_components
            ]:
                errors.append(
                    f"{bootstrap_path}: release bootstrap component manifest must "
                    f"equal {expected_bootstrap_components!r}"
                )
            if bootstrap_assignments["_BOOTSTRAP_COMPONENT_SHA256"] != [
                expected_bootstrap_component_sha256
            ]:
                errors.append(
                    f"{bootstrap_path}: release bootstrap component digests must "
                    f"equal {expected_bootstrap_component_sha256!r}"
                )
            terminal_keys = bootstrap_assignments["_TERMINAL_EVIDENCE_KEYS"]
            if (
                len(terminal_keys) != 1
                or not isinstance(terminal_keys[0], set)
                or "formal_replay_release" not in terminal_keys[0]
            ):
                errors.append(
                    f"{bootstrap_path}: terminal release evidence must require the "
                    "signed formal replay release bundle"
                )
            replay_validator_options = (
                "--formal-replay-source-receipt",
                "--formal-replay-release-root",
                "--expected-formal-replay-signature-sha256",
                "--formal-replay-principal",
            )
            validator_orders = bootstrap_assignments["_VALIDATOR_OPTION_ORDER"]
            if len(validator_orders) != 1 or not isinstance(
                validator_orders[0], tuple
            ):
                errors.append(
                    f"{bootstrap_path}: validator option order must remain literal"
                )
            else:
                validator_order = validator_orders[0]
                try:
                    replay_offset = validator_order.index(
                        "--formal-replay-source-receipt"
                    )
                except ValueError:
                    replay_offset = -1
                if validator_order[
                    replay_offset : replay_offset + len(replay_validator_options)
                ] != replay_validator_options:
                    errors.append(
                        f"{bootstrap_path}: protected receipt validation must carry "
                        "the four formal replay options in canonical order"
                    )
            validator_paths = bootstrap_assignments["_VALIDATOR_PATH_OPTIONS"]
            if (
                len(validator_paths) != 1
                or not isinstance(validator_paths[0], frozenset)
                or not set(replay_validator_options[:2]).issubset(
                    validator_paths[0]
                )
                or set(replay_validator_options[2:]) & validator_paths[0]
            ):
                errors.append(
                    f"{bootstrap_path}: formal replay validator path/text kinds "
                    "must remain exact"
                )
            expected_parent_bootstrap_symbols = frozenset(
                symbol
                for symbols in expected_bootstrap_component_symbols.values()
                for symbol in symbols
            )
            parent_bootstrap_symbols = tuple(
                statement.name
                for statement in bootstrap_tree.body
                if isinstance(statement, (ast.FunctionDef, ast.AsyncFunctionDef))
                and statement.name in expected_parent_bootstrap_symbols
            )
            if parent_bootstrap_symbols:
                errors.append(
                    f"{bootstrap_path}: receipt-replay bootstrap functions must "
                    "remain isolated in the declared component"
                )
            for component_name in expected_bootstrap_components:
                component_path = bootstrap_path.with_name(component_name)
                if component_path.is_symlink() or not component_path.is_file():
                    errors.append(
                        f"{component_path}: release bootstrap component must be a "
                        "regular non-symlink file"
                    )
                    continue
                try:
                    component_tree = ast.parse(
                        component_path.read_text(encoding="utf-8"),
                        filename=str(component_path),
                    )
                except (OSError, UnicodeDecodeError, SyntaxError) as error:
                    errors.append(
                        f"{component_path}: release bootstrap component is invalid: "
                        f"{error}"
                    )
                    continue
                component_symbols = tuple(
                    statement.name
                    for statement in component_tree.body
                    if isinstance(statement, (ast.FunctionDef, ast.AsyncFunctionDef))
                )
                if component_symbols != expected_bootstrap_component_symbols[component_name]:
                    errors.append(
                        f"{component_path}: release bootstrap component symbols must "
                        f"equal {expected_bootstrap_component_symbols[component_name]!r}"
                    )
                component_sha256 = hashlib.sha256(component_path.read_bytes()).hexdigest()
                if component_sha256 != expected_bootstrap_component_sha256[component_name]:
                    errors.append(
                        f"{component_path}: release bootstrap component SHA-256 must "
                        f"equal {expected_bootstrap_component_sha256[component_name]}"
                    )
                component_source = component_path.read_text(encoding="utf-8")
                for fragment, description in (
                    (
                        'receipt_evidence["formal_replay_release"]',
                        "the terminal signed replay evidence",
                    ),
                    (
                        '"terminal formal replay finalized root",\n',
                        "the exact finalized bundle inventory",
                    ),
                    (
                        'finalized["receipt"].sha256 != source_receipt.sha256\n',
                        "the source/archive receipt equality gate",
                    ),
                    (
                        '"tlapm-projection/Folds.tla",\n',
                        "the authenticated read-only Folds projection",
                    ),
                    (
                        '"tlapm-projection/Functions.tla",\n',
                        "the authenticated read-only Functions projection",
                    ),
                    (
                        '"terminal formal replay finalized TLAPM projection",\n',
                        "the exact two-file projection inventory",
                    ),
                    (
                        '"--expected-formal-replay-signature-sha256",\n',
                        "the protected signature digest replay",
                    ),
                ):
                    if component_source.count(fragment) != 1:
                        errors.append(
                            f"{component_path}: bootstrap formal replay integration "
                            f"must retain {description} exactly once"
                        )

    for assignment in (
        'required_data_model_status_test="block::consensus_v2::tests::'
        'status_validation_accepts_all_ignore_reasons_and_rejects_a_thirteenth_entry"',
        'required_data_model_lane_certificate_test="block::consensus::tests::'
        'lane_block_certificate_decodes_atomically_from_slice"',
    ):
        if source.splitlines().count(assignment) != 1:
            errors.append(
                f"{release_path}: required data-model contract must be pinned exactly: "
                f"{assignment}"
            )
    if source.count("lane-certificate-rust cargo-exact 1") != 1:
        errors.append(
            f"{release_path}: atomic lane-certificate decode must retain one exact leg"
        )

    documentation_claims = {
        repo_root / "formal" / "sumeragi_v2" / "README.md": (
            "current inventory to 867 tests across 44 modules.\n"
            "Together with the source-sealed command and tooling legs, the pre-network\n"
            f"corridor contains {_PRODUCTION_LIVENESS_RELEASE_CORRIDOR_LEG_COUNT} legs.",
            "canonical module/test TSV inventory SHA-256 is\n"
            f"`{_PRODUCTION_LIVENESS_RELEASE_INVENTORY_SHA256}`",
        ),
        repo_root / "formal" / "sumeragi_v2" / "PROOF.md": (
            "current 867-test,\n44-module inventory. The complete source-sealed\n"
            "pre-network corridor\ncontains "
            f"{_PRODUCTION_LIVENESS_RELEASE_CORRIDOR_LEG_COUNT} legs.",
            "canonical module/test TSV inventory SHA-256 is\n"
            f"`{_PRODUCTION_LIVENESS_RELEASE_INVENTORY_SHA256}`",
        ),
        repo_root / "specs" / "sumeragi_v2_liveness.md": (
            "current inventory to 867\nexact tests across 44 modules and "
            f"{_PRODUCTION_LIVENESS_RELEASE_CORRIDOR_LEG_COUNT} pre-network legs.",
            "Its canonical module/test TSV inventory SHA-256 is\n"
            f"`{_PRODUCTION_LIVENESS_RELEASE_INVENTORY_SHA256}`",
        ),
        repo_root / "specs" / "sumeragi_v2_multilane_closure_ledger.md": (
            "`terminal_sweep_source_partitions_whole_units_before_any_mutation` in\n"
            "`crates/iroha_core/src/sumeragi/tests/"
            "v2_runner_lifecycle_startup_order.rs`,",
            "source anchors are not the complete "
            f"{_PRODUCTION_MULTILANE_FOCUS_TEST_COUNT}-test `G-UNIT` receipt",
            "tests are mapped row evidence, not the complete "
            f"{_PRODUCTION_MULTILANE_FOCUS_TEST_COUNT}-test `G-UNIT` receipt",
            "contain exactly "
            f"{_PRODUCTION_MULTILANE_FOCUS_TEST_COUNT} unique required\n"
            "tests: 316 core, 143 queue-journal, 13 configuration, eight data-model,\n"
            "39 Torii, one Torii-shared, and two integration.",
            "both require that exact\n"
            f"{_PRODUCTION_MULTILANE_FOCUS_TEST_COUNT}-row shape",
            "The G-UNIT static inventory checks establish exact "
            f"`{_PRODUCTION_MULTILANE_FOCUS_TEST_COUNT}/"
            f"{_PRODUCTION_MULTILANE_FOCUS_TEST_COUNT}` source consistency",
            "execution of all "
            f"{_PRODUCTION_MULTILANE_FOCUS_TEST_COUNT} required tests",
            "remains Open until the exact no-skip suites run through the compliant "
            "isolated\nwrapper",
            "no complete "
            f"{_PRODUCTION_MULTILANE_FOCUS_TEST_COUNT}-test execution from an "
            "immutable candidate is\n  claimed by this reconciliation.",
        ),
    }
    for path, claims in documentation_claims.items():
        if not path.is_file() or path.is_symlink():
            errors.append(f"{path}: release inventory documentation must be regular")
            continue
        documentation = path.read_text(encoding="utf-8")
        for claim in claims:
            if documentation.count(claim) != 1:
                errors.append(
                    f"{path}: release inventory documentation must contain exact "
                    f"claim {claim!r}"
                )
    closure_ledger_path = (
        repo_root / "specs" / "sumeragi_v2_multilane_closure_ledger.md"
    )
    if closure_ledger_path.is_file() and not closure_ledger_path.is_symlink():
        closure_ledger = closure_ledger_path.read_text(encoding="utf-8")
        for stale_claim in (
            "terminal_sweep_source_binds_chain_route_and_empty_post_readback",
            "474-test",
            "474-row",
            "`474/474`",
            "268 core",
        ):
            if stale_claim in closure_ledger:
                errors.append(
                    f"{closure_ledger_path}: multilane closure ledger retains "
                    f"stale release claim {stale_claim!r}"
                )
    return errors


def _promotion_target_evidence_errors(
    evidence: dict[str, Any],
    *,
    formal_dir: Path = FORMAL_DIR,
    root_dir: Path = ROOT_DIR,
) -> list[str]:
    """Validate the canonical ordered strict transcript for every 9 + 3 target."""

    errors: list[str] = []
    if evidence.get("schema_version") != EVIDENCE_SCHEMA_VERSION:
        errors.append(
            f"proof evidence schema_version must equal {EVIDENCE_SCHEMA_VERSION}"
        )
    if evidence.get("protocol") != "sumeragi-v2":
        errors.append("promotion targets require protocol sumeragi-v2")
    if evidence.get("backend_verification") is not True:
        errors.append("promotion targets require backend-verified TLAPS evidence")
    expected_tool = {
        "name": "TLAPM",
        "commit": TLAPM_COMMIT,
        "version": TLAPM_COMMIT[:7],
    }
    if evidence.get("tool") != expected_tool:
        errors.append("promotion targets require the exact pinned TLAPM identity")
    expected_manifest = _formal_source_manifest(formal_dir, root_dir)
    if evidence.get("source_manifest") != expected_manifest:
        errors.append(
            "promotion targets are stale relative to the current formal sources"
        )
    source_manifest_sha256 = expected_manifest["sha256"]
    try:
        ledger_sha256 = _proof_ledger_sha256(formal_dir)
    except (OSError, ValueError) as error:
        errors.append(f"promotion targets cannot bind the proof ledger: {error}")
        ledger_sha256 = ""
    if evidence.get("ledger_sha256") != ledger_sha256:
        errors.append(
            "promotion targets are stale relative to the byte-exact proof ledger"
        )
    try:
        expected_targets = _promotion_target_entries(formal_dir, root_dir)
    except (OSError, UnicodeDecodeError, ValueError) as error:
        return errors + [f"promotion target contract cannot be resolved: {error}"]
    targets = evidence.get("promotion_targets")
    if not isinstance(targets, list):
        return errors + ["proof evidence promotion_targets must be an array"]
    observed_ids = [
        entry.get("obligation_id") if isinstance(entry, dict) else None
        for entry in targets
    ]
    expected_ids = [entry["obligation_id"] for entry in expected_targets]
    if observed_ids != expected_ids:
        errors.append(
            "proof evidence promotion targets must preserve canonical 9 + 3 "
            f"order; expected {expected_ids!r}, found {observed_ids!r}"
        )
    if len({value for value in observed_ids if isinstance(value, str)}) != len(
        [value for value in observed_ids if isinstance(value, str)]
    ):
        errors.append("proof evidence promotion targets must not repeat an ID")
    if len(targets) != len(expected_targets):
        return errors
    dynamic_fields = {
        "obligations_proved",
        "log",
        "log_sha256",
        "source_manifest_sha256",
        "ledger_sha256",
    }
    for index, expected_target in enumerate(expected_targets):
        entry = targets[index]
        obligation_id = expected_target["obligation_id"]
        if not isinstance(entry, dict):
            errors.append(
                f"proof evidence promotion target {obligation_id} must be an object"
            )
            continue
        if set(entry) != set(expected_target) | dynamic_fields:
            errors.append(
                f"proof evidence promotion target {obligation_id} fields are not "
                "canonical"
            )
        for field, expected_value in expected_target.items():
            if entry.get(field) != expected_value:
                errors.append(
                    f"proof evidence promotion target {obligation_id} has wrong "
                    f"{field}"
                )
        proved = entry.get("obligations_proved")
        if not isinstance(proved, int) or isinstance(proved, bool) or proved <= 0:
            errors.append(
                f"proof evidence promotion target {obligation_id} has no positive "
                "proved count"
            )
        frozen_count = expected_target["expected_obligations"]
        if frozen_count is not None and proved != frozen_count:
            errors.append(
                f"proof evidence promotion target {obligation_id} does not match "
                f"frozen obligation count {frozen_count}"
            )
        if entry.get("source_manifest_sha256") != source_manifest_sha256:
            errors.append(
                f"proof evidence promotion target {obligation_id} is not bound "
                "to the current source manifest"
            )
        if entry.get("ledger_sha256") != ledger_sha256:
            errors.append(
                f"proof evidence promotion target {obligation_id} is not bound "
                "to the current proof ledger"
            )
        expected_log = _formal_evidence_logical_path(
            "tlaps", "targets", f"{obligation_id}.log"
        )
        if entry.get("log") != expected_log:
            errors.append(
                f"proof evidence promotion target {obligation_id} must use log "
                f"{expected_log}"
            )
            continue
        log_path = _formal_evidence_physical_path(expected_log, root_dir)
        if not log_path.is_file() or log_path.is_symlink():
            errors.append(
                f"proof evidence target log is not a regular file: {log_path}"
            )
            continue
        if entry.get("log_sha256") != _sha256_file(log_path):
            errors.append(
                f"proof evidence target log digest mismatch for {obligation_id}"
            )
            continue
        try:
            log_source = log_path.read_text(encoding="utf-8")
        except UnicodeDecodeError:
            errors.append(f"proof evidence target log is not UTF-8: {log_path}")
            continue
        actual_count = _tlapm_target_obligation_count(
            log_source,
            target=expected_target,
            source_manifest_sha256=source_manifest_sha256,
            ledger_sha256=ledger_sha256,
        )
        if actual_count is None:
            errors.append(
                "proof evidence target log lacks the exact range-, source-, and "
                f"ledger-bound successful suffix for {obligation_id}"
            )
        if actual_count != proved:
            errors.append(
                f"proof evidence target proved count does not match log for "
                f"{obligation_id}"
            )
    return errors


def _release_evidence_errors(
    ledger: dict[str, Any],
    evidence: dict[str, Any] | None,
    *,
    formal_dir: Path = FORMAL_DIR,
    root_dir: Path = ROOT_DIR,
    require_global_completion: bool = True,
) -> list[str]:
    errors: list[str] = []
    if (
        require_global_completion
        and ledger.get("machine_checked_completion") is not True
    ):
        errors.append("release gate requires machine_checked_completion=true")

    obligations = ledger.get("obligations")
    if isinstance(obligations, list):
        if (
            require_global_completion
            and ledger.get("machine_checked_completion") is True
        ):
            errors.extend(
                _machine_checked_completion_status_errors(obligations)
            )
        if require_global_completion:
            for obligation in obligations:
                if not isinstance(obligation, dict):
                    continue
                status = obligation.get("status")
                if (
                    status == "specified_unproved"
                    and obligation.get("id")
                    in _MACHINE_CHECKED_COMPLETION_TARGET_ID_SET
                ):
                    errors.append(
                        "release gate rejects unproved target obligation: "
                        f"{obligation.get('id', '<unknown>')}"
                    )

    if evidence is None:
        return errors + ["release gate requires fresh TLAPS proof evidence"]
    if not isinstance(evidence, dict):
        return errors + ["proof evidence must be a JSON object"]
    expected_top_level_keys = {
        "schema_version",
        "protocol",
        "backend_verification",
        "tool",
        "source_manifest",
        "ledger_sha256",
        "modules",
        "promotion_targets",
        "facade_providers",
    }
    if set(evidence) != expected_top_level_keys:
        errors.append(
            "proof evidence fields must equal "
            f"{sorted(expected_top_level_keys)}, found {sorted(evidence)}"
        )
    if evidence.get("schema_version") != EVIDENCE_SCHEMA_VERSION:
        errors.append(f"proof evidence schema_version must equal {EVIDENCE_SCHEMA_VERSION}")
    if evidence.get("protocol") != "sumeragi-v2":
        errors.append("proof evidence protocol must equal sumeragi-v2")
    if evidence.get("backend_verification") is not True:
        errors.append("release gate requires backend-verified TLAPS evidence")

    tool = evidence.get("tool")
    if not isinstance(tool, dict):
        errors.append("proof evidence tool must be an object")
    else:
        if set(tool) != {"name", "commit", "version"}:
            errors.append("proof evidence tool fields must be name, commit, and version")
        if tool.get("name") != "TLAPM":
            errors.append("proof evidence must identify TLAPM")
        if tool.get("commit") != TLAPM_COMMIT:
            errors.append(f"proof evidence must use pinned TLAPM commit {TLAPM_COMMIT}")
        version = tool.get("version")
        if version != TLAPM_COMMIT[:7]:
            errors.append(
                f"proof evidence TLAPM version must equal {TLAPM_COMMIT[:7]}"
            )

    expected_manifest = _formal_source_manifest(formal_dir, root_dir)
    if evidence.get("source_manifest") != expected_manifest:
        errors.append("proof evidence source manifest does not match current TLA+ sources")
    source_manifest_sha256 = expected_manifest["sha256"]
    canonical_ledger_path = formal_dir / "proof_coverage.json"
    try:
        canonical_ledger = load_ledger(canonical_ledger_path)
        expected_ledger_sha256 = _proof_ledger_sha256(formal_dir)
    except (OSError, json.JSONDecodeError, DuplicateKeyError, ValueError) as error:
        errors.append(f"proof evidence cannot resolve source-bound ledger: {error}")
        expected_ledger_sha256 = ""
    else:
        if canonical_ledger != ledger:
            errors.append(
                "release proof ledger differs from the source-bound canonical "
                "proof_coverage.json"
            )
    if evidence.get("ledger_sha256") != expected_ledger_sha256:
        errors.append(
            "proof evidence ledger digest does not match byte-exact "
            "proof_coverage.json"
        )
    errors.extend(
        _promotion_target_evidence_errors(
            evidence, formal_dir=formal_dir, root_dir=root_dir
        )
    )

    modules = evidence.get("modules")
    if not isinstance(modules, list):
        errors.append("proof evidence modules must be an array")
        return errors
    observed: list[str] = []
    for entry in modules:
        if not isinstance(entry, dict):
            errors.append("proof evidence module entries must be objects")
            continue
        if set(entry) != {
            "module",
            "obligations_proved",
            "preflight_log",
            "preflight_log_sha256",
            "log",
            "log_sha256",
            "source_manifest_sha256",
            "ledger_sha256",
        }:
            errors.append("proof evidence module fields are not canonical")
        module = entry.get("module")
        proved = entry.get("obligations_proved")
        if not _nonempty_string(module):
            errors.append("proof evidence module is missing a name")
            continue
        if module not in RELEASE_PROOF_MODULES:
            errors.append(f"proof evidence contains unknown module {module!r}")
            continue
        if module in observed:
            errors.append(f"proof evidence repeats module {module}")
        observed.append(module)
        if not isinstance(proved, int) or isinstance(proved, bool) or proved <= 0:
            errors.append(f"proof evidence module {module} has no positive proved count")
        if entry.get("source_manifest_sha256") != source_manifest_sha256:
            errors.append(
                f"proof evidence module {module} is not bound to the current source manifest"
            )
        if entry.get("ledger_sha256") != expected_ledger_sha256:
            errors.append(
                f"proof evidence module {module} is not bound to the current "
                "proof ledger"
            )

        preflight_value = entry.get("preflight_log")
        expected_preflight = _formal_evidence_logical_path(
            "tlaps", f"{module}.preflight.log"
        )
        if preflight_value != expected_preflight:
            errors.append(
                f"proof evidence module {module} must use preflight log "
                f"{expected_preflight}"
            )
        else:
            preflight_path = _formal_evidence_physical_path(
                expected_preflight, root_dir
            )
            if not preflight_path.is_file() or preflight_path.is_symlink():
                errors.append(
                    f"proof evidence preflight log is not a regular file: {preflight_path}"
                )
            elif entry.get("preflight_log_sha256") != _sha256_file(preflight_path):
                errors.append(f"proof evidence preflight log digest mismatch for {module}")
            else:
                try:
                    preflight_source = preflight_path.read_text(encoding="utf-8")
                except UnicodeDecodeError:
                    errors.append(
                        f"proof evidence preflight log is not UTF-8: {preflight_path}"
                    )
                else:
                    if not _valid_tlapm_preflight_log(
                        preflight_source,
                        module=module,
                        source_manifest_sha256=source_manifest_sha256,
                        ledger_sha256=expected_ledger_sha256,
                    ):
                        errors.append(
                            "proof evidence preflight log lacks the exact "
                            f"manifest-bound successful suffix for {module}"
                        )

        log_value = entry.get("log")
        expected_log = _formal_evidence_logical_path("tlaps", f"{module}.log")
        if log_value != expected_log:
            errors.append(f"proof evidence module {module} must use log {expected_log}")
            continue
        log_path = _formal_evidence_physical_path(expected_log, root_dir)
        if not log_path.is_file() or log_path.is_symlink():
            errors.append(f"proof evidence log is not a regular file: {log_path}")
            continue
        actual_log_sha256 = _sha256_file(log_path)
        if entry.get("log_sha256") != actual_log_sha256:
            errors.append(f"proof evidence log digest mismatch for {module}")
            continue
        try:
            log_source = log_path.read_text(encoding="utf-8")
        except UnicodeDecodeError:
            errors.append(f"proof evidence log is not UTF-8: {log_path}")
            continue
        actual_count = _tlapm_obligation_count(
            log_source,
            module=module,
            source_manifest_sha256=source_manifest_sha256,
            ledger_sha256=expected_ledger_sha256,
        )
        if actual_count is None:
            errors.append(
                f"proof evidence log lacks the exact manifest-bound successful suffix for {module}"
            )
        if actual_count != proved:
            errors.append(f"proof evidence proved count does not match log for {module}")
    if observed != list(RELEASE_PROOF_MODULES):
        errors.append(
            "proof evidence must cover the release proof modules in canonical order; "
            f"expected {list(RELEASE_PROOF_MODULES)}, found {observed}"
        )
    try:
        expected_providers = _facade_provider_entries(formal_dir, root_dir)
    except (OSError, ValueError, json.JSONDecodeError, DuplicateKeyError) as error:
        errors.append(f"could not resolve async liveness facade providers: {error}")
    else:
        if evidence.get("facade_providers") != expected_providers:
            errors.append(
                "proof evidence async liveness facade providers do not match "
                "the current ordered shard contract"
            )
    return errors
