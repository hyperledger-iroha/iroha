# Lexically loaded by check_sumeragi_v2_multilane_models.py.

def _validate_inflight_layout_contract(
    root: Path,
    formal_dir: Path,
    contract: Any,
    errors: list[str],
) -> None:
    """Bind the in-flight corpus and composed relation without trace-theorem inflation."""

    expected_keys = {
        "claim",
        "module",
        "positive_config",
        "runner",
        "evidence",
        "required_actions",
        "required_invariants",
        "mutations",
        "production_symbols",
        "ordered_source_checks",
        "forbidden_source_checks",
        "source_checks",
        "forbidden_tokens",
    }
    if not isinstance(contract, dict) or set(contract) != expected_keys:
        errors.append(
            "in-flight layout contract must contain exactly claim, module, "
            "positive_config, runner, evidence, required_actions, "
            "required_invariants, mutations, production_symbols, "
            "ordered_source_checks, forbidden_source_checks, source_checks, "
            "and forbidden_tokens"
        )
        return
    expected_scalars = {
        "claim": INFLIGHT_LAYOUT_CLAIM,
        "module": INFLIGHT_LAYOUT_MODULE,
        "positive_config": INFLIGHT_LAYOUT_POSITIVE_CONFIG,
        "runner": INFLIGHT_LAYOUT_RUNNER.as_posix(),
        "evidence": INFLIGHT_LAYOUT_EVIDENCE.as_posix(),
    }
    for field, expected in expected_scalars.items():
        if contract.get(field) != expected:
            errors.append(
                f"in-flight layout contract {field} must equal {expected!r}"
            )

    required_actions = contract.get("required_actions")
    if (
        not isinstance(required_actions, list)
        or tuple(required_actions) != INFLIGHT_LAYOUT_REQUIRED_ACTIONS
    ):
        errors.append(
            "in-flight layout contract actions differ from the exact reviewed "
            "current-semantics inventory"
        )

    required_invariants = contract.get("required_invariants")
    if (
        not isinstance(required_invariants, list)
        or tuple(required_invariants) != INFLIGHT_LAYOUT_REQUIRED_INVARIANTS
    ):
        errors.append(
            "in-flight layout contract invariants differ from the exact reviewed "
            "current-layout inventory"
        )

    mutations = contract.get("mutations")
    actual_mutations: list[tuple[str, str]] = []
    if not isinstance(mutations, list):
        errors.append("in-flight layout mutations must be an array")
    else:
        for mutation in mutations:
            if not isinstance(mutation, dict) or set(mutation) != {
                "config",
                "invariant",
            }:
                errors.append(
                    "each in-flight layout mutation must contain only config "
                    "and invariant"
                )
                continue
            config = mutation.get("config")
            invariant = mutation.get("invariant")
            if not _nonempty_string(config) or not _nonempty_string(invariant):
                errors.append(f"malformed in-flight layout mutation {mutation!r}")
                continue
            actual_mutations.append((config, invariant))
    if tuple(actual_mutations) != INFLIGHT_LAYOUT_MUTATIONS:
        errors.append(
            "in-flight layout mutation mapping differs from the exact reviewed "
            "twenty-two-control corpus"
        )

    production_symbols = contract.get("production_symbols")
    actual_bindings: list[tuple[str, str, str, tuple[str, ...]]] = []
    if not isinstance(production_symbols, list):
        errors.append("in-flight production_symbols must be an array")
    else:
        for binding in production_symbols:
            if not isinstance(binding, dict) or set(binding) != {
                "path",
                "kind",
                "symbol",
                "required_tokens",
            }:
                errors.append(
                    "each in-flight production binding must contain only path, "
                    "kind, symbol, and required_tokens"
                )
                continue
            relative = binding.get("path")
            kind = binding.get("kind")
            symbol = binding.get("symbol")
            tokens = binding.get("required_tokens")
            if (
                not _nonempty_string(relative)
                or kind not in RUST_BINDING_KINDS
                or not _nonempty_string(symbol)
                or not isinstance(tokens, list)
                or not tokens
                or not all(_nonempty_string(token) for token in tokens)
                or len(tokens) != len(set(tokens))
            ):
                errors.append(f"malformed in-flight production binding {binding!r}")
                continue
            actual_bindings.append(
                _current_inflight_production_binding(
                    (relative, kind, symbol, tuple(tokens))
                )
            )
    if tuple(actual_bindings) != INFLIGHT_LAYOUT_PRODUCTION_BINDINGS:
        errors.append(
            "in-flight production bindings differ from the exact reviewed "
            "payload/queue/Kura/replay-state layout contract"
        )

    ordered_source_checks = contract.get("ordered_source_checks")
    actual_ordered: list[tuple[str, str, str, tuple[str, ...]]] = []
    if not isinstance(ordered_source_checks, list):
        errors.append("in-flight ordered_source_checks must be an array")
    else:
        for check in ordered_source_checks:
            if not isinstance(check, dict) or set(check) != {
                "path",
                "kind",
                "symbol",
                "tokens",
            }:
                errors.append(
                    "each ordered in-flight source check must contain only path, "
                    "kind, symbol, and tokens"
                )
                continue
            relative = check.get("path")
            kind = check.get("kind")
            symbol = check.get("symbol")
            tokens = check.get("tokens")
            if (
                not _nonempty_string(relative)
                or kind not in RUST_BINDING_KINDS
                or not _nonempty_string(symbol)
                or not isinstance(tokens, list)
                or not tokens
                or not all(_nonempty_string(token) for token in tokens)
                or len(tokens) != len(set(tokens))
            ):
                errors.append(f"malformed ordered in-flight source check {check!r}")
                continue
            actual_ordered.append(
                _INFLIGHT_CURRENT_ORDERED_BINDINGS.get(
                    (relative, symbol),
                    (relative, kind, symbol, tuple(tokens)),
                )
            )
    if tuple(actual_ordered) != INFLIGHT_LAYOUT_ORDERED_SOURCE_CHECKS:
        errors.append(
            "in-flight ordered source checks differ from the exact reviewed "
            "validation/durability/publication order contract"
        )

    forbidden_source_checks = contract.get("forbidden_source_checks")
    actual_forbidden_source_checks: list[
        tuple[str, str, str, tuple[str, ...]]
    ] = []
    if not isinstance(forbidden_source_checks, list):
        errors.append("in-flight forbidden_source_checks must be an array")
    else:
        for check in forbidden_source_checks:
            if not isinstance(check, dict) or set(check) != {
                "path",
                "kind",
                "symbol",
                "forbidden_tokens",
            }:
                errors.append(
                    "each forbidden in-flight source check must contain only "
                    "path, kind, symbol, and forbidden_tokens"
                )
                continue
            relative = check.get("path")
            kind = check.get("kind")
            symbol = check.get("symbol")
            tokens = check.get("forbidden_tokens")
            if (
                not _nonempty_string(relative)
                or kind not in RUST_BINDING_KINDS
                or not _nonempty_string(symbol)
                or not isinstance(tokens, list)
                or not tokens
                or not all(_nonempty_string(token) for token in tokens)
                or len(tokens) != len(set(tokens))
            ):
                errors.append(f"malformed forbidden in-flight source check {check!r}")
                continue
            actual_forbidden_source_checks.append(
                (relative, kind, symbol, tuple(tokens))
            )
    if (
        tuple(actual_forbidden_source_checks)
        != INFLIGHT_LAYOUT_FORBIDDEN_SOURCE_CHECKS
    ):
        errors.append(
            "in-flight forbidden source checks differ from the exact reviewed "
            "bounded-application/capability contract"
        )

    source_checks = contract.get("source_checks")
    actual_source_checks: list[tuple[str, tuple[str, ...]]] = []
    if not isinstance(source_checks, list):
        errors.append("in-flight source_checks must be an array")
    else:
        for check in source_checks:
            if not isinstance(check, dict) or set(check) != {
                "path",
                "required_tokens",
            }:
                errors.append(
                    "each in-flight source check must contain only path and "
                    "required_tokens"
                )
                continue
            relative = check.get("path")
            tokens = check.get("required_tokens")
            if (
                not _nonempty_string(relative)
                or not isinstance(tokens, list)
                or not tokens
                or not all(_nonempty_string(token) for token in tokens)
                or len(tokens) != len(set(tokens))
            ):
                errors.append(f"malformed in-flight source check {check!r}")
                continue
            actual_source_checks.append((relative, tuple(tokens)))
    if tuple(actual_source_checks) != INFLIGHT_LAYOUT_SOURCE_CHECKS:
        errors.append(
            "in-flight whole-file source checks differ from the exact reviewed "
            "version/bound/runner contract"
        )

    forbidden_tokens = contract.get("forbidden_tokens")
    if (
        not isinstance(forbidden_tokens, list)
        or tuple(forbidden_tokens) != INFLIGHT_LAYOUT_FORBIDDEN_TOKENS
    ):
        errors.append(
            "in-flight forbidden tokens differ from the exact reviewed stale-layout "
            "inventory"
        )

    module_path = formal_dir / f"{INFLIGHT_LAYOUT_MODULE}.tla"
    module_source: str | None = None
    if _regular_file(module_path, "in-flight first-release TLA+ module", errors):
        module_source = module_path.read_text(encoding="utf-8")
        header = MODULE_RE.search(module_source)
        if header is None or header.group(1) != INFLIGHT_LAYOUT_MODULE:
            errors.append(
                f"{module_path}: module header must declare {INFLIGHT_LAYOUT_MODULE}"
            )
        if not module_source.rstrip().endswith("===="):
            errors.append(f"{module_path}: module must end with ====")
        if "ProductionRefinementObligation" in module_source:
            errors.append(
                f"{module_path}: bounded kernel without production trace extraction "
                "must not declare a production refinement obligation"
            )
        for action in INFLIGHT_LAYOUT_REQUIRED_ACTIONS:
            declaration_re = re.compile(
                TLA_DECLARATION_TEMPLATE.format(symbol=re.escape(action))
            )
            count = len(tuple(declaration_re.finditer(module_source)))
            if count != 1:
                errors.append(
                    f"{module_path}: current-semantics action {action} must be "
                    f"declared exactly once, found {count}"
                )
        for invariant in INFLIGHT_LAYOUT_REQUIRED_INVARIANTS:
            declaration_re = re.compile(
                TLA_DECLARATION_TEMPLATE.format(symbol=re.escape(invariant))
            )
            count = len(tuple(declaration_re.finditer(module_source)))
            if count != 1:
                errors.append(
                    f"{module_path}: current-layout invariant {invariant} must be "
                    f"declared exactly once, found {count}"
                )
        for token in INFLIGHT_COMPOSED_TLA_ALIGNMENT_TOKENS:
            count = module_source.count(token)
            if count != 1:
                errors.append(
                    f"{module_path}: composed Rust/TLA action-alignment token "
                    f"{token!r} must occur exactly once, found {count}"
                )

    positive_path = formal_dir / INFLIGHT_LAYOUT_POSITIVE_CONFIG
    positive_source: str | None = None
    if _regular_file(
        positive_path, "in-flight first-release positive TLC config", errors
    ):
        positive_source = positive_path.read_text(encoding="utf-8")
        if not positive_source.startswith("INIT Init\nNEXT Next\n"):
            errors.append(
                f"{positive_path}: positive config must use executable Init/Next"
            )
        positive_invariants = tuple(
            re.findall(r"(?m)^INVARIANT ([A-Za-z0-9_]+)$", positive_source)
        )
        if positive_invariants != INFLIGHT_LAYOUT_REQUIRED_INVARIANTS:
            errors.append(
                f"{positive_path}: invariant list differs from the exact reviewed "
                "current-layout contract"
            )

    mutation_sources: list[tuple[Path, str]] = []
    for config, invariant in INFLIGHT_LAYOUT_MUTATIONS:
        config_path = formal_dir / config
        if not _regular_file(
            config_path, "in-flight first-release mutation TLC config", errors
        ):
            continue
        config_source = config_path.read_text(encoding="utf-8")
        mutation_sources.append((config_path, config_source))
        config_invariants = tuple(
            re.findall(r"(?m)^INVARIANT ([A-Za-z0-9_]+)$", config_source)
        )
        if config_invariants != ("FirstReleaseTypeInvariant", invariant):
            errors.append(
                f"{config_path}: mutation must check exactly the type invariant "
                f"and {invariant}"
            )

    runner_path = root / INFLIGHT_LAYOUT_RUNNER
    runner_source: str | None = None
    if _regular_file(
        runner_path, "in-flight first-release TLC mutation runner", errors
    ):
        if runner_path.stat().st_mode & 0o111 == 0:
            errors.append(
                f"in-flight first-release runner must be executable: {runner_path}"
            )
        runner_source = runner_path.read_text(encoding="utf-8")
        runner_calls = tuple(
            re.findall(
                r"(?m)^run_mutant ([a-z0-9_]+_bug\.cfg) ([A-Za-z0-9_]+)$",
                runner_source,
            )
        )
        if runner_calls != INFLIGHT_LAYOUT_MUTATIONS:
            errors.append(
                f"{runner_path}: mutation calls differ from the exact reviewed "
                "twenty-two-control corpus"
            )
        compact_runner_source = " ".join(
            runner_source.replace("\\\n", " ").split()
        )
        for token in (
            'source "${REPO_ROOT}/scripts/formal/sumeragi_v2_tlc_result_contract.sh"',
            '[[ "$status" -ne 12 ]]',
            'local invariant_marker="Error: Invariant ${invariant} is violated."',
            'sumeragi_v2_tlc_assert_exact_line "$config" "$log" '
            '"$invariant_marker"',
            'sumeragi_v2_tlc_assert_exact_line "$config" "$log" '
            '"Error: The behavior up to this point is:"',
            'sumeragi_v2_tlc_assert_terminal "$config" "$log"',
            "bounded abstract evidence only; no production refinement claim",
        ):
            count = compact_runner_source.count(token)
            if count != 1:
                errors.append(
                    f"{runner_path}: fail-closed runner token {token!r} must occur "
                    f"exactly once, found {count}"
                )
        if runner_source.count("\nrun_positive\n") != 1:
            errors.append(
                f"{runner_path}: fail-closed runner must invoke the positive "
                "model exactly once"
            )

    evidence_path = root / INFLIGHT_LAYOUT_EVIDENCE
    evidence_source: str | None = None
    if _regular_file(
        evidence_path, "in-flight first-release evidence boundary", errors
    ):
        evidence_source = evidence_path.read_text(encoding="utf-8")
        for token in (
            "`LaneExecutablePayloadV1`",
            "`LANE_EXECUTABLE_PAYLOAD_VERSION_V1`",
            "QueuePlan journal V1",
            "reservation journal V1",
            "selected-batch conjunction",
            "READY signature",
            "atomic WSV carrier application",
            "four-stage release",
            "twenty-two `_bug.cfg`",
            "`composed_state_action_relation_with_source_bound_trace_extraction`",
            "fixed-width composed state/action relation",
            "production trace-extraction theorem",
            "reverse terminal-owner projection",
        ):
            if token not in evidence_source:
                errors.append(
                    f"{evidence_path}: missing current-layout evidence token {token!r}"
                )

    closure_path = root / CLOSURE_LEDGER_RELATIVE
    closure_source: str | None = None
    if _regular_file(
        closure_path, "multilane closure ledger for in-flight layout", errors
    ):
        closure_source = closure_path.read_text(encoding="utf-8")
        for token in (
            "Current first-release layouts are source-bound",
            "`LaneExecutablePayloadV1`",
            "QueuePlan journal V1",
            "reservation journal V1",
            "selected-batch",
            "READY authorization/signature/QC",
            "post-carrier",
            "four-stage",
            "`composed_state_action_relation_with_source_bound_trace_extraction`",
            "fixed-width composed transition relation is implemented",
            "production trace-extraction certificate",
            "twenty-two exact TLC mutation witnesses",
        ):
            if token not in closure_source:
                errors.append(
                    f"{closure_path}: missing current-layout closure token {token!r}"
                )

    stale_surfaces: list[tuple[Path, str]] = []
    if module_source is not None:
        stale_surfaces.append((module_path, module_source))
    if positive_source is not None:
        stale_surfaces.append((positive_path, positive_source))
    stale_surfaces.extend(mutation_sources)
    if runner_source is not None:
        stale_surfaces.append((runner_path, runner_source))
    if evidence_source is not None:
        stale_surfaces.append((evidence_path, evidence_source))
    if closure_source is not None:
        stale_surfaces.append((closure_path, closure_source))
    for path, source in stale_surfaces:
        for token in INFLIGHT_LAYOUT_FORBIDDEN_TOKENS:
            if token in source:
                errors.append(
                    f"{path}: stale first-release layout token {token!r} is forbidden"
                )

    binding_items: dict[tuple[str, str, str], str] = {}
    for relative, kind, symbol, tokens in INFLIGHT_LAYOUT_PRODUCTION_BINDINGS:
        item = _rust_binding_item(
            root,
            relative,
            kind,
            symbol,
            "in-flight production layout binding",
            errors,
        )
        if item is None:
            continue
        binding_items[(relative, kind, symbol)] = item
        for token in tokens:
            if token not in item:
                errors.append(
                    f"{root / relative}: in-flight production item {symbol} is "
                    f"missing current-layout token {token!r}"
                )

    for relative, kind, symbol, tokens in INFLIGHT_LAYOUT_ORDERED_SOURCE_CHECKS:
        item = binding_items.get((relative, kind, symbol))
        if item is None:
            item = _rust_binding_item(
                root,
                relative,
                kind,
                symbol,
                "ordered in-flight production layout binding",
                errors,
            )
        if item is None:
            continue
        cursor = -1
        for token in tokens:
            position = item.find(token, cursor + 1)
            if position < 0:
                errors.append(
                    f"{root / relative}: ordered in-flight item {symbol} is "
                    f"missing or reorders token {token!r}"
                )
                break
            cursor = position

    for relative, kind, symbol, tokens in INFLIGHT_LAYOUT_FORBIDDEN_SOURCE_CHECKS:
        item = binding_items.get((relative, kind, symbol))
        if item is None:
            item = _rust_binding_item(
                root,
                relative,
                kind,
                symbol,
                "forbidden in-flight production source binding",
                errors,
            )
        if item is None:
            continue
        for token in tokens:
            if token in item:
                errors.append(
                    f"{root / relative}: in-flight production item {symbol} "
                    f"contains forbidden source-bound token {token!r}"
                )

    for relative, tokens in INFLIGHT_LAYOUT_SOURCE_CHECKS:
        path, source = _read_reviewed_rust_source(
            root, relative, "in-flight whole-file source binding", errors
        )
        if source is None:
            continue
        for token in tokens:
            count = source.count(token)
            if count != 1:
                errors.append(
                    f"{path}: current-layout token {token!r} must occur exactly "
                    f"once, found {count}"
                )

    _regular_file(
        root / INFLIGHT_LAYOUT_TEST,
        "in-flight layout negative-control test",
        errors,
    )


