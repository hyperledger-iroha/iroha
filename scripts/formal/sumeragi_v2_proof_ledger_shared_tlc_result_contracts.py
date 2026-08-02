def _shared_tlc_result_contract_source_fidelity_errors(
    repo_root: Path = ROOT_DIR,
) -> list[str]:
    """Seal the common nonzero, success, diagnostic, and footer contract."""

    errors: list[str] = []
    expected_paths = {
        SHARED_TLC_RESULT_CONTRACT,
        *SHARED_TLC_RESULT_CONTRACT_CALLERS,
    }
    expected_callers = set(SHARED_TLC_RESULT_CONTRACT_CALLERS)
    expected_specialized_callers = {
        "scripts/formal/check_sumeragi_v2_replay_trace.sh",
        "scripts/formal/run_sumeragi_v2_item_carrier_typing_mutation.sh",
        "scripts/formal/run_sumeragi_v2_liveness_ownership_mutations.sh",
        "scripts/formal/run_sumeragi_v2_tlc.sh",
    }
    specialized_callers = set(SHARED_TLC_RESULT_SPECIALIZED_CALLERS)
    if (
        len(SHARED_TLC_RESULT_CONTRACT_CALLERS) != 32
        or len(expected_callers) != 32
    ):
        errors.append(
            "shared TLC result-contract caller inventory must contain "
            "exactly thirty-two unique callers"
        )
    if (
        len(SHARED_TLC_RESULT_SPECIALIZED_CALLERS) != 4
        or specialized_callers != expected_specialized_callers
        or not specialized_callers <= expected_callers
    ):
        errors.append(
            "shared TLC result-contract specialized complement must equal "
            "the replay, item-carrier, liveness-ownership, and aggregate runners"
        )

    observed_callers: set[str] = set()
    formal_scripts_dir = repo_root / "scripts" / "formal"
    if not formal_scripts_dir.is_dir():
        errors.append(
            f"{formal_scripts_dir}: shared TLC caller census directory is missing"
        )
    else:
        census_paths = list(
            formal_scripts_dir.glob("run_sumeragi_v2_*.sh")
        )
        census_paths.append(
            formal_scripts_dir / "check_sumeragi_v2_replay_trace.sh"
        )
        for path in census_paths:
            if not path.is_file():
                continue
            try:
                source = path.read_text(encoding="utf-8")
            except OSError as error:
                errors.append(
                    f"{path}: shared TLC caller census could not read source: "
                    f"{error}"
                )
                continue
            if "tlc2.TLC" in source:
                observed_callers.add(
                    path.relative_to(repo_root).as_posix()
                )
    if observed_callers != expected_callers:
        errors.append(
            "shared TLC result-contract caller census must classify every "
            "TLC-launching Sumeragi runner exactly once; "
            f"unclassified={sorted(observed_callers - expected_callers)}, "
            f"missing={sorted(expected_callers - observed_callers)}"
        )

    digest_paths = set(SHARED_TLC_RESULT_CONTRACT_SHA256)
    if digest_paths != expected_paths:
        errors.append(
            "shared TLC result-contract digest inventory must equal the "
            f"helper and exact thirty-two callers; "
            f"missing={sorted(expected_paths - digest_paths)}, "
            f"extra={sorted(digest_paths - expected_paths)}"
        )
    profile_keys = set(SHARED_TLC_RESULT_BRANCH_PROFILES)
    if profile_keys != expected_callers:
        errors.append(
            "shared TLC result-contract branch-profile inventory must equal "
            f"the exact caller census; missing={sorted(expected_callers - profile_keys)}, "
            f"extra={sorted(profile_keys - expected_callers)}"
        )
    assertion_profile_keys = set(
        SHARED_TLC_RESULT_ASSERTION_SITE_PROFILES
    )
    if assertion_profile_keys != expected_callers:
        errors.append(
            "shared TLC result-contract assertion-site inventory must equal "
            "the exact caller census; "
            f"missing={sorted(expected_callers - assertion_profile_keys)}, "
            f"extra={sorted(assertion_profile_keys - expected_callers)}"
        )
    branch_profile_digest = hashlib.sha256(
        json.dumps(
            SHARED_TLC_RESULT_BRANCH_PROFILES,
            sort_keys=True,
            separators=(",", ":"),
        ).encode("utf-8")
    ).hexdigest()
    if (
        branch_profile_digest
        != SHARED_TLC_RESULT_BRANCH_PROFILES_SHA256
    ):
        errors.append(
            "shared TLC result-contract branch profiles must retain their "
            "canonical sealed inventory"
        )
    assertion_profile_digest = hashlib.sha256(
        json.dumps(
            SHARED_TLC_RESULT_ASSERTION_SITE_PROFILES,
            sort_keys=True,
            separators=(",", ":"),
        ).encode("utf-8")
    ).hexdigest()
    if (
        assertion_profile_digest
        != SHARED_TLC_RESULT_ASSERTION_SITE_PROFILES_SHA256
    ):
        errors.append(
            "shared TLC result-contract assertion-site profiles must retain "
            "their canonical sealed inventory"
        )
    for relative, profile in SHARED_TLC_RESULT_BRANCH_PROFILES.items():
        if (
            not isinstance(profile, tuple)
            or len(profile) != 5
            or any(
                not isinstance(count, int) or count < 0
                for count in profile
            )
            or sum(profile) == 0
        ):
            errors.append(
                f"{relative}: shared TLC branch profile must contain five "
                "nonnegative counts and at least one reviewed branch"
            )
    for relative, profile in (
        SHARED_TLC_RESULT_ASSERTION_SITE_PROFILES.items()
    ):
        if (
            not isinstance(profile, tuple)
            or len(profile) != 5
            or any(
                not isinstance(count, int) or count < 0
                for count in profile
            )
        ):
            errors.append(
                f"{relative}: shared TLC assertion-site profile must contain "
                "five nonnegative counts"
            )
            continue
        primary_site_count = profile[4]
        if (relative in specialized_callers) != (
            primary_site_count == 0
        ):
            errors.append(
                f"{relative}: shared TLC primary-diagnostic assertion site "
                "must be absent exactly for the four specialized callers"
            )

    sources: dict[str, str] = {}
    for relative in sorted(expected_paths):
        path = repo_root / relative
        if not path.is_file() or path.is_symlink():
            errors.append(
                f"{path}: shared TLC result-contract source must be a regular file"
            )
            continue
        if (
            relative in SHARED_TLC_RESULT_CONTRACT_CALLERS
            and not os.access(path, os.X_OK)
        ):
            errors.append(
                f"{path}: shared TLC result-contract caller must be executable"
            )
        sources[relative] = path.read_text(encoding="utf-8")
        expected_sha256 = SHARED_TLC_RESULT_CONTRACT_SHA256.get(relative)
        if expected_sha256 is None:
            continue
        observed_sha256 = _sha256_file(path)
        if observed_sha256 != expected_sha256:
            errors.append(
                f"{path}: shared TLC result-contract source SHA-256 must equal "
                f"{expected_sha256}; found {observed_sha256}"
            )

    helper_source = sources.get(SHARED_TLC_RESULT_CONTRACT)
    if helper_source is None:
        return errors
    exact_helper_constants = (
        (
            "readonly SUMERAGI_V2_TLC_FINISHED_PATTERN="
            "'^Finished in (([0-9]+d )?([0-9]+h )?"
            "([0-9]+min )?[0-9]+(ms|s)|([0-9]+d )?"
            "([0-9]+h )?[0-9]+min|([0-9]+d )?[0-9]+h|"
            "[0-9]+d) at \\([0-9]{4}-[0-9]{2}-[0-9]{2} "
            "[0-9]{2}:[0-9]{2}:[0-9]{2}\\)$'",
            "anchored terminal-footer pattern",
        ),
        (
            'readonly SUMERAGI_V2_TLC_SUCCESS_MARKER="Model checking '
            'completed. No error has been found."',
            "exact fixed-success marker",
        ),
        (
            "readonly SUMERAGI_V2_TLC_STATE_SUMMARY_PATTERN="
            "'^[0-9][0-9,]* states generated, [0-9][0-9,]* "
            "distinct states found, [0-9][0-9,]* states left on "
            "queue[.]$'",
            "anchored full state-summary pattern",
        ),
        (
            "readonly SUMERAGI_V2_TLC_STATE_SUMMARY_PREFIX="
            "'^[0-9][0-9,]* states generated, [0-9][0-9,]* "
            "distinct states found'",
            "anchored state-summary candidate prefix",
        ),
        (
            "readonly SUMERAGI_V2_TLC_FAILURE_DIAGNOSTIC_PATTERN="
            "'^[[:space:]]*(Error:|Deadlock reached([.]|$)|"
            "Temporal properties were violated[.]$)'",
            "leading-whitespace failure-diagnostic pattern",
        ),
        (
            "readonly SUMERAGI_V2_TLC_PRIMARY_DIAGNOSTIC_PATTERN="
            "'^[[:space:]]*(Error: (Invariant |Action property |"
            "Temporal properties were violated[.]$|"
            "Deadlock reached([.]|$))|Deadlock reached([.]|$)|"
            "Temporal properties were violated[.]$)'",
            "exact primary-diagnostic pattern",
        ),
    )
    for token, description in exact_helper_constants:
        if helper_source.count(token) != 1:
            errors.append(
                f"{repo_root / SHARED_TLC_RESULT_CONTRACT}: shared TLC "
                f"result contract must retain exactly one {description}"
            )

    helper_headers = (
        "sumeragi_v2_tlc_contract_fail() {",
        "sumeragi_v2_tlc_assert_regular_log() {",
        "sumeragi_v2_tlc_assert_nonzero_state_space() {",
        "sumeragi_v2_tlc_assert_terminal() {",
        "sumeragi_v2_tlc_assert_exact_line() {",
        "sumeragi_v2_tlc_assert_fixed_success() {",
    )
    helper_offsets = [helper_source.find(header) for header in helper_headers]
    if (
        any(helper_source.count(header) != 1 for header in helper_headers)
        or any(offset < 0 for offset in helper_offsets)
        or helper_offsets != sorted(helper_offsets)
    ):
        errors.append(
            f"{repo_root / SHARED_TLC_RESULT_CONTRACT}: shared TLC result "
            "contract must retain the exact ordered helper-function inventory"
        )
        return errors
    helper_sections = {
        header: helper_source[
            offset : (
                helper_offsets[index + 1]
                if index + 1 < len(helper_offsets)
                else len(helper_source)
            )
        ]
        for index, (header, offset) in enumerate(
            zip(helper_headers, helper_offsets)
        )
    }

    regular_log_section = helper_sections[
        "sumeragi_v2_tlc_assert_regular_log() {"
    ]
    for token, description in (
        (
            'if [[ ! -f "$log" || -L "$log" ]]; then',
            "regular non-symlink log guard",
        ),
        (
            '"TLC log must be a fresh regular file"',
            "fresh-log failure diagnostic",
        ),
    ):
        if regular_log_section.count(token) != 1:
            errors.append(
                f"{repo_root / SHARED_TLC_RESULT_CONTRACT}: shared TLC "
                f"regular-log helper must retain exactly one {description}"
            )

    nonzero_section = helper_sections[
        "sumeragi_v2_tlc_assert_nonzero_state_space() {"
    ]
    for token, description in (
        (
            'grep -E "$SUMERAGI_V2_TLC_STATE_SUMMARY_PREFIX" "$log" |\n'
            "      tail -n 1 || true",
            "last state-summary candidate parser",
        ),
        (
            'grep -Eq "$SUMERAGI_V2_TLC_STATE_SUMMARY_PATTERN" '
            '<<<"$state_line" || {',
            "anchored full state-summary validation",
        ),
        (
            "if ((generated <= 0 || distinct <= 0)); then",
            "positive generated/distinct state-count guard",
        ),
    ):
        if nonzero_section.count(token) != 1:
            errors.append(
                f"{repo_root / SHARED_TLC_RESULT_CONTRACT}: shared TLC "
                f"nonzero-state helper must retain exactly one {description}"
            )

    terminal_section = helper_sections[
        "sumeragi_v2_tlc_assert_terminal() {"
    ]
    for token, description in (
        (
            'grep -Ec "$SUMERAGI_V2_TLC_FINISHED_PATTERN" "$log" || true',
            "anchored terminal-marker counter",
        ),
        (
            '[[ "$terminal_count" == 1 ]] || {',
            "single terminal-marker cardinality guard",
        ),
        (
            "last_nonblank=\"$(awk 'NF { line = $0 } END { print line }' "
            '"$log")"',
            "last-nonblank footer extraction",
        ),
        (
            '"$SUMERAGI_V2_TLC_FINISHED_PATTERN" '
            '<<<"$last_nonblank" || {',
            "terminal footer position guard",
        ),
    ):
        if terminal_section.count(token) != 1:
            errors.append(
                f"{repo_root / SHARED_TLC_RESULT_CONTRACT}: shared TLC "
                f"terminal helper must retain exactly one {description}"
            )

    exact_line_section = helper_sections[
        "sumeragi_v2_tlc_assert_exact_line() {"
    ]
    for token, description in (
        (
            'marker_count="$(grep -Fxc "$marker" "$log" || true)"',
            "full-line marker counter",
        ),
        (
            '[[ "$marker_count" == 1 ]] || {',
            "single full-line marker cardinality guard",
        ),
    ):
        if exact_line_section.count(token) != 1:
            errors.append(
                f"{repo_root / SHARED_TLC_RESULT_CONTRACT}: shared TLC "
                f"exact-line helper must retain exactly one {description}"
            )

    fixed_success_section = helper_sections[
        "sumeragi_v2_tlc_assert_fixed_success() {"
    ]
    for token, description in (
        (
            '[[ "$actual_status" -eq 0 ]] || {',
            "zero-status guard",
        ),
        (
            'sumeragi_v2_tlc_assert_nonzero_state_space "$label" "$log"',
            "nonzero-state assertion",
        ),
        (
            'sumeragi_v2_tlc_assert_exact_line \\\n'
            '    "$label" "$log" "$SUMERAGI_V2_TLC_SUCCESS_MARKER"',
            "exact success-marker assertion",
        ),
        (
            'grep -Ec "$SUMERAGI_V2_TLC_FAILURE_DIAGNOSTIC_PATTERN" '
            '"$log" ||\n      true',
            "leading-whitespace failure-diagnostic counter",
        ),
        (
            '[[ "$failure_count" == 0 ]] || {',
            "zero failure-diagnostic guard",
        ),
        (
            'sumeragi_v2_tlc_assert_terminal "$label" "$log"',
            "terminal-footer assertion",
        ),
    ):
        if fixed_success_section.count(token) != 1:
            errors.append(
                f"{repo_root / SHARED_TLC_RESULT_CONTRACT}: shared TLC "
                f"fixed-success helper must retain exactly one {description}"
            )

    exact_source_invocation = (
        'source "${REPO_ROOT}/scripts/formal/'
        'sumeragi_v2_tlc_result_contract.sh"'
    )
    liveness = (
        "scripts/formal/run_sumeragi_v2_liveness_ownership_mutations.sh"
    )
    replay = "scripts/formal/check_sumeragi_v2_replay_trace.sh"
    liveness_source_invocation = 'source "$TLC_RESULT_CONTRACT"'
    repo_root_assignment = (
        'readonly REPO_ROOT="$(cd -- "$(dirname -- '
        '"${BASH_SOURCE[0]}")/../.." && pwd)"'
    )
    replay_repo_root_assignment = (
        'REPO_ROOT="$(cd -- "$(dirname -- '
        '"${BASH_SOURCE[0]}")/../.." && pwd)"'
    )
    aggregate = "scripts/formal/run_sumeragi_v2_tlc.sh"
    assertion_site_tokens = (
        "sumeragi_v2_tlc_assert_fixed_success",
        "sumeragi_v2_tlc_assert_nonzero_state_space",
        "sumeragi_v2_tlc_assert_exact_line",
        "sumeragi_v2_tlc_assert_terminal",
        "SUMERAGI_V2_TLC_PRIMARY_DIAGNOSTIC_PATTERN",
    )
    for relative in SHARED_TLC_RESULT_CONTRACT_CALLERS:
        caller_source = sources.get(relative)
        if caller_source is None:
            continue
        path = repo_root / relative
        source_invocation = (
            liveness_source_invocation
            if relative == liveness
            else exact_source_invocation
        )
        root_assignment = (
            replay_repo_root_assignment
            if relative == replay
            else repo_root_assignment
        )
        source_offset = caller_source.find(source_invocation)
        repo_root_offset = caller_source.find(root_assignment)
        contract_call_matches = list(
            re.finditer(
                r"(?<![A-Za-z0-9_])sumeragi_v2_tlc_assert_"
                r"[A-Za-z0-9_]+(?=\s)",
                caller_source,
            )
        )
        first_contract_call = (
            contract_call_matches[0].start()
            if contract_call_matches
            else -1
        )
        execution_offsets = [
            offset
            for offset in (
                caller_source.find("resolved_java_bin="),
                caller_source.find("tlc2.TLC"),
                first_contract_call,
            )
            if offset >= 0
        ]
        if (
            caller_source.count(source_invocation) != 1
            or caller_source.count(root_assignment) != 1
            or repo_root_offset < 0
            or source_offset <= repo_root_offset
            or (
                execution_offsets
                and source_offset >= min(execution_offsets)
            )
        ):
            errors.append(
                f"{path}: shared TLC result contract must be sourced exactly "
                "once after REPO_ROOT and before Java/TLC validation"
            )
        if (
            relative == replay
            and caller_source.count(
                f"{replay_repo_root_assignment}\nreadonly REPO_ROOT"
            )
            != 1
        ):
            errors.append(
                f"{path}: replay TLC runner must freeze REPO_ROOT before "
                "sourcing the shared result contract"
            )
        if relative == liveness:
            for token, description in (
                (
                    "readonly TLC_RESULT_CONTRACT="
                    '"${REPO_ROOT}/scripts/formal/'
                    'sumeragi_v2_tlc_result_contract.sh"',
                    "immutable shared-helper path",
                ),
                (
                    '[[ -f "$TLC_RESULT_CONTRACT" ]] || {',
                    "shared-helper regular-path preflight",
                ),
                (
                    "# shellcheck source=sumeragi_v2_tlc_result_contract.sh",
                    "shellcheck shared-helper declaration",
                ),
            ):
                if caller_source.count(token) != 1:
                    errors.append(
                        f"{path}: specialized liveness TLC runner must "
                        f"retain exactly one {description}"
                    )
        if relative == aggregate:
            adjacent_source = (
                f"{repo_root_assignment}\n{exact_source_invocation}\n"
            )
            if caller_source.count(adjacent_source) != 1:
                errors.append(
                    f"{path}: aggregate TLC runner must source the shared "
                    "result contract immediately after REPO_ROOT"
                )

        expected_assertion_sites = (
            SHARED_TLC_RESULT_ASSERTION_SITE_PROFILES.get(relative)
        )
        observed_assertion_sites = tuple(
            len(
                re.findall(
                    rf"(?<![A-Za-z0-9_]){re.escape(token)}"
                    rf"(?![A-Za-z0-9_])",
                    caller_source,
                )
            )
            for token in assertion_site_tokens
        )
        if observed_assertion_sites != expected_assertion_sites:
            errors.append(
                f"{path}: shared TLC assertion-site profile must equal "
                f"{expected_assertion_sites}; found {observed_assertion_sites}"
            )
        if (
            first_contract_call < 0
            or source_offset < 0
            or first_contract_call <= source_offset
        ):
            errors.append(
                f"{path}: shared TLC result assertions must execute only "
                "after the helper is sourced"
            )

        branch_profile = SHARED_TLC_RESULT_BRANCH_PROFILES.get(relative)
        if (
            branch_profile is not None
            and branch_profile[3] > 0
            and "Temporal properties were violated." not in caller_source
        ):
            errors.append(
                f"{path}: temporal mutation branches must retain the "
                "canonical TLC temporal primary diagnostic"
            )

        if relative not in specialized_callers:
            variable_primary_guard = re.search(
                r'\[\[\s+"\$(?:[A-Za-z_][A-Za-z0-9_]*_)?'
                r'primary_diagnostic_count"\s+-eq\s+1\s+\]\]'
                r"\s+\|\|\s+\{",
                caller_source,
            )
            inline_primary_guard = re.search(
                r'\[\[\s*"\$\('
                r"(?:(?!\)\").)*"
                r"SUMERAGI_V2_TLC_PRIMARY_DIAGNOSTIC_PATTERN"
                r"(?:(?!\)\").)*"
                r'\)"\s*==\s*1\s*\]\]\s*\|\|\s*\{',
                caller_source,
                flags=re.DOTALL,
            )
            if (
                variable_primary_guard is None
                and inline_primary_guard is None
            ):
                errors.append(
                    f"{path}: non-specialized TLC mutation runner must "
                    "require exactly one primary failure diagnostic"
                )

    replay_source = sources.get(replay)
    if replay_source is not None:
        for token, description in (
            (
                'if [[ "$tlc_status" -ne 12 || ! -s "$tlc_log" ]]; then',
                "status-12 nonempty witness guard",
            ),
            (
                'sumeragi_v2_tlc_assert_regular_log '
                '"replay-decision-witness" "$tlc_log"',
                "fresh regular witness-log assertion",
            ),
            (
                'if ! python3 "$NORMALIZER" "$tlc_log" --seed "$SEED" '
                '>"$normalized_trace"; then',
                "strict trace normalizer invocation",
            ),
            (
                'if ! cmp -s "$EXPECTED" "$normalized_trace"; then',
                "checked-in trace comparison",
            ),
            (
                'bash "$REPO_ROOT/scripts/formal/'
                'run_sumeragi_v2_harness.sh" --model-replay',
                "production reducer replay",
            ),
        ):
            if replay_source.count(token) != 1:
                errors.append(
                    f"{repo_root / replay}: specialized replay TLC runner "
                    f"must retain exactly one {description}"
                )

    item_carrier = (
        "scripts/formal/run_sumeragi_v2_item_carrier_typing_mutation.sh"
    )
    item_carrier_source = sources.get(item_carrier)
    if item_carrier_source is not None:
        item_diagnostic_pattern = (
            "'^[[:space:]]*(Error:|Deadlock reached([.]|$)|"
            "Temporal properties were violated[.]$)'"
        )
        item_contract_tokens = (
            (
                '[[ "$actual_status" -ne "$expected_status" ]]',
                1,
                "exact expected-status comparison",
            ),
            (
                item_diagnostic_pattern,
                2,
                "success/configuration failure-diagnostic scans",
            ),
            (
                '[[ "$diagnostic_count" -eq 0 ]] || {',
                1,
                "successful-run zero-diagnostic guard",
            ),
            (
                '[[ "$diagnostic_count" -eq 1 ]] || {',
                1,
                "configuration-failure single-diagnostic guard",
            ),
            (
                "item_carrier_typing_loose_proposal_source.cfg 151",
                1,
                "status-151 configuration-evaluation mutation",
            ),
            (
                "Error: The invariant of "
                "SelectedTypingImpliesCanonicalProposalCarrier is equal "
                "to FALSE",
                1,
                "exact configuration-evaluation diagnostic",
            ),
        )
        for token, expected_count, description in item_contract_tokens:
            observed_count = item_carrier_source.count(token)
            if observed_count != expected_count:
                errors.append(
                    f"{repo_root / item_carrier}: specialized item-carrier "
                    f"TLC runner must retain {expected_count} {description}; "
                    f"found {observed_count}"
                )

    liveness_source = sources.get(liveness)
    if liveness_source is not None:
        liveness_contract_tokens = (
            (
                'if [[ "$expected_status" -eq 12 ]]; then',
                "status-12 classifier",
            ),
            (
                'elif [[ "$expected_status" -eq 13 ]]; then',
                "status-13 classifier",
            ),
            (
                '[[ "$expected_primary" =~ '
                "^Error:\\ Invariant\\ .+\\ is\\ violated\\.$ ]]",
                "named-invariant primary validation",
            ),
            (
                '[[ "$expected_primary" == '
                '"Error: Temporal properties were violated." ]]',
                "canonical temporal primary validation",
            ),
            (
                '[[ "$diagnostic_count" == 2 ]] || {',
                "exact primary-plus-behavior diagnostic guard",
            ),
            (
                '[[ "$whitespace_prefixed_count" == 0 ]] || {',
                "zero whitespace-prefixed diagnostic guard",
            ),
            (
                '[[ "$#" == 1 ]] || {',
                "one-declared-primary guard",
            ),
            (
                'assert_mutation_failure_contract \\\n'
                '      "$label" "$log" "$expected_status" "$1"',
                "central failure-contract invocation",
            ),
        )
        for token, description in liveness_contract_tokens:
            if liveness_source.count(token) != 1:
                errors.append(
                    f"{repo_root / liveness}: specialized liveness TLC "
                    f"runner must retain exactly one {description}"
                )
        for token, expected_count, description in (
            (
                "'^[[:space:]]*(Error:|Deadlock reached([.]|$)|"
                "Temporal properties were violated[.]$)'",
                1,
                "all-diagnostic scan",
            ),
            (
                "'^[[:space:]]+(Error:|Deadlock reached([.]|$)|"
                "Temporal properties were violated[.]$)'",
                1,
                "whitespace-spoof scan",
            ),
        ):
            observed_count = liveness_source.count(token)
            if observed_count != expected_count:
                errors.append(
                    f"{repo_root / liveness}: specialized liveness TLC "
                    f"runner must retain {expected_count} {description}; "
                    f"found {observed_count}"
                )

    aggregate_source = sources.get(aggregate)
    if aggregate_source is not None:
        aggregate_assertion_counts = {
            "sumeragi_v2_tlc_assert_nonzero_state_space": 1,
            "sumeragi_v2_tlc_assert_exact_line": 2,
            "sumeragi_v2_tlc_assert_terminal": 2,
        }
        for assertion, expected_count in aggregate_assertion_counts.items():
            observed_count = len(
                re.findall(
                    rf"(?<![A-Za-z0-9_]){re.escape(assertion)}"
                    rf"(?![A-Za-z0-9_])",
                    aggregate_source,
                )
            )
            if observed_count != expected_count:
                errors.append(
                    f"{repo_root / aggregate}: aggregate TLC component "
                    f"assertion {assertion} must occur {expected_count} "
                    f"times; found {observed_count}"
                )
        for token, description in (
            (
                "/^[[:space:]]*(Error:|Deadlock reached([.]|$)|"
                "Temporal properties were violated[.]$)/ &&",
                "locked-Commit unexpected-diagnostic scan",
            ),
            (
                '$0 != "Error: Invariant '
                'NoRecoveredHistoricalLockedCommitSigning is violated." &&',
                "locked-Commit named invariant exception",
            ),
            (
                '$0 != "Error: The behavior up to this point is:" {',
                "locked-Commit behavior diagnostic exception",
            ),
            (
                '[[ "$witness_forbidden_diagnostic_count" == 0 ]] || {',
                "locked-Commit zero unexpected-diagnostic guard",
            ),
            (
                'grep -Ec "$SUMERAGI_V2_TLC_FAILURE_DIAGNOSTIC_PATTERN" \\\n'
                '        "$tlc_log" || true',
                "simulation error/deadlock counter",
            ),
            (
                '|| "$error_count" != 0 ]]; then',
                "simulation zero error/deadlock guard",
            ),
        ):
            if aggregate_source.count(token) != 1:
                errors.append(
                    f"{repo_root / aggregate}: aggregate TLC runner must "
                    f"retain exactly one {description}"
                )
    return errors
