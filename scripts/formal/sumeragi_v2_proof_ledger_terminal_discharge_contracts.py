# Executed lexically in check_sumeragi_v2_proof_ledger.py; do not import directly.

REPLAY_TRACE_SOURCE_SHA256 = {
    "formal/sumeragi_v2/SumeragiV2TraceWitness.tla": (
        "c8d716f4dfdfd92618fc68967348b04260904d2afd581682a5a1be48386c3328"
    ),
    "crates/iroha_sumeragi_core/tests/fixtures/tlc_replay_witness.cfg": (
        "36c2822579c247c95384e0a337fd287a4bc35e56c7860a3c93043f6d3b9b2d14"
    ),
    "crates/iroha_sumeragi_core/tests/fixtures/tlc_replay_witness.tsv": (
        "32b3a409c731cb7c9619d1f8d6ee74e8b6bce666766dae557c04de7b3394b748"
    ),
    "scripts/normalize_sumeragi_v2_tlc_trace.py": (
        "e21f876507ce5152a8e64a07cfcff235bc9709d294800f77130e819deccd3537"
    ),
    "scripts/formal/check_sumeragi_v2_replay_trace.sh": (
        "ccdae44c2fc9d12dfe7a1239988de036a3181aea7f76c522a83ce4809f300118"
    ),
    "crates/iroha_sumeragi_core/tests/model_trace_replay.rs": (
        "87305ef2c045f815b18d9bac556285fe15813b4b3ff801f3c1f19c7272a55349"
    ),
    "scripts/formal/run_sumeragi_v2_harness.sh": (
        "d3637e08cd97783506adb397a97885ac27daa4633abccdd38fbe7a47b78eeac1"
    ),
}


REPLAY_MODEL_TESTS = (
    "tlc_liveness_witness_replays_against_the_production_reducer",
    "identical_commit_envelope_stutters_before_lock_and_is_admitted_after_persistence",
    "malformed_and_unsafe_normalized_traces_fail_closed",
    "crash_replay_rejects_stale_completion_and_resumes_exact_intent",
    "unsafe_certificate_and_vote_equivocation_do_not_decide",
    "invalid_body_never_authorizes_prepare_or_decision",
    "overlapping_timeout_groups_are_rejected_transactionally",
    "timeout_equivocation_with_different_full_high_qcs_is_reported",
)


_DEFERRED_QC_OWNERSHIP_RUST_ITEM_SHA256 = {
    "reducer_qc_matches_wire": (
        "1e8f6891257f678e9a6b576d2caccae61dd2a69e92ea8a1eabe407e86568942d"
    ),
    "deferred_quorum_certificate_owner_tag": (
        "712d669930de9f0505460a6e7ff554ca4e06acbdf824b3edff454ba575f722c5"
    ),
    "deferred_quorum_certificate_owner": (
        "caf5ad2bdb1efb8bae103e6dc03be79047260fc8c22c26b136df6375cfebcfa8"
    ),
}


_PRODUCTION_P2P_REPLY_TENURE_ITEM_SHA256 = {
    "ReliableReplyRouteTenure::is_active": (
        "e009b05d84e02598af9f6bbd8b661f85d56718fd8b4d7edf36324c9f5559db48"
    ),
    "ReliableReplyRouteTenure::is_reply_writable": (
        "347bda104a8d6ca9c8fac2cc2771568d0d64a8563eb9359f89851ee73444ba51"
    ),
    "ReliableReplyRouteTenure::mark_draining": (
        "a70e629ee89cbd8c31b22d20fc74a34d4c13117984a25924003556b9770c875a"
    ),
    "ReliableReplyRouteTenure::mark_termination_seen": (
        "6a52b2c481ae88caadd87ccd5138ffd477fd7b0d50ec4934f2b06585d9ac9732"
    ),
    "ReliableReplyRouteTenure::cancel": (
        "6f71f12d88a206b7dede19f3c261536da53408d02aea40a8c2f195af492deef5"
    ),
    "ProgressDeliveryAuthority::is_active": (
        "16b74ee70e4dd09c2c7f5014cdd08d453adc70fdf41afac7079b813838db1aac"
    ),
}


_PRODUCTION_P2P_DELIVERY_DRAIN_ITEM_SHA256 = {
    "InboundDeliveryDrain::new": (
        "7940f7e38da3b094be1e6016df85ee576e8710642ecb3178f78b155ff256bcc9"
    ),
    "InboundDeliveryDrain::register": (
        "637a1066a544d29044484d5a4607a8553fb81e155d7172fccc47e25056f1724f"
    ),
    "InboundDeliveryDrain::close_producer": (
        "c02965ccadda92972ea0c7991f76747cd6c7e64a95933094342b73be52d193fc"
    ),
    "InboundDeliveryDrain::is_complete": (
        "5cfec68ae97e8211f8c64021676d2beb8b060ba039decbf776697a7b7d6f8242"
    ),
    "InboundDeliveryDrain::wait_complete": (
        "e45df3ca7109198dbb45aa1cc779e7e1a884b1dcdc8ff543db08fa9e891342c3"
    ),
    "InboundDeliveryDrainGuard::drop": (
        "201cd5209d79e2a70c675a89576c9e72bb487fee8c0f8259455988d3a8bce0da"
    ),
    "PeerMessageSenders::transfer_before_send": (
        "433c29f88ba37edbf7c10888d7c04299d86c0e066c395934858f7cfa5f2d014e"
    ),
    "PeerMessage::attach_delivery_drain": (
        "401a55a52d65ff41276c1b66b8a50d9b652cdccf88558e28478470cd3b19d501"
    ),
    "PeerMessage::into_parts_with_reply_route": (
        "8aaf0e03409e4c04d1b019b483a4d0ed29d415eb0e2796313be5ae00abcb3f6c"
    ),
}


_PRODUCTION_P2P_REPLY_ROUTE_RECEIPT_ITEM_SHA256 = {
    "NetworkReplyRoutesPruneReceipt::into_output": (
        "ac5b98dae5bbb3d21e61f39d0a6edf7d64025a98a308bc48c159a1e6847abe5b"
    ),
    "NetworkReplyRoutesMergeTransition::into_output": (
        "7090f93dd2bb5a4aea996664cdf29d20d231d380c4e0065ffeea313b9e518174"
    ),
    "NetworkReplyRoutesStrictMergeReceipt::into_output": (
        "a409349ab25756ee2e47cfbf893bd64c7c83d316b33625449bfaed0e40ac965e"
    ),
    "NetworkReplyRoutesObservedMergeReceipt::into_output": (
        "a409349ab25756ee2e47cfbf893bd64c7c83d316b33625449bfaed0e40ac965e"
    ),
}


def _replay_trace_source_fidelity_errors(
    repo_root: Path = ROOT_DIR,
) -> list[str]:
    """Seal and connect every deterministic TLC replay trust link."""

    errors: list[str] = []
    sources: dict[str, str] = {}
    for relative, expected_sha256 in REPLAY_TRACE_SOURCE_SHA256.items():
        path = repo_root / relative
        if not path.is_file() or path.is_symlink():
            errors.append(f"{path}: replay trace source must be a regular file")
            continue
        actual_sha256 = _sha256_file(path)
        if actual_sha256 != expected_sha256:
            errors.append(
                f"{path}: replay trace source must match exact reviewed SHA-256 "
                f"{expected_sha256}; found {actual_sha256}"
            )
        try:
            sources[relative] = path.read_text(encoding="utf-8")
        except UnicodeDecodeError:
            errors.append(f"{path}: replay trace source must be UTF-8")

    def require_once(relative: str, fragment: str, description: str) -> None:
        source = sources.get(relative)
        if source is None:
            return
        count = source.count(fragment)
        if count != 1:
            errors.append(
                f"{repo_root / relative}: replay trace source must contain "
                f"{description} exactly once; found {count}"
            )

    witness_relative = "formal/sumeragi_v2/SumeragiV2TraceWitness.tla"
    for fragment, description in (
        ("---- MODULE SumeragiV2TraceWitness ----\n", "the witness module header"),
        ("EXTENDS SumeragiV2\n", "the production-model extension"),
        (
            "WitnessSpec ==\n"
            "  WitnessInit /\\ [][WitnessNextV2]_WitnessVars "
            "/\\ WitnessActionFairness\n",
            "the exact fair witness specification",
        ),
        ("NoDecision == decisions = {}\n", "the non-vacuous decision witness"),
    ):
        require_once(witness_relative, fragment, description)

    config_relative = (
        "crates/iroha_sumeragi_core/tests/fixtures/tlc_replay_witness.cfg"
    )
    for fragment, description in (
        ("SPECIFICATION WitnessSpec\n", "the WitnessSpec selection"),
        ("CHECK_DEADLOCK FALSE\n", "the finite-witness deadlock policy"),
        ("INVARIANT TypeInvariant\n", "the type invariant"),
        ("INVARIANT DecisionAgreement\n", "the agreement invariant"),
        ("INVARIANT NoDecision\n", "the intentional decision violation"),
        ("  N = 4\n", "the four-validator geometry"),
    ):
        require_once(config_relative, fragment, description)

    fixture_relative = (
        "crates/iroha_sumeragi_core/tests/fixtures/tlc_replay_witness.tsv"
    )
    fixture_source = sources.get(fixture_relative)
    if fixture_source is not None:
        lines = fixture_source.splitlines()
        expected_header = [
            "# sumeragi-v2-tlc-action-trace-v1",
            "# seed=19349663",
            "# step\taction\tnode\tpeer\tview\tphase\tsubject",
        ]
        if lines[:3] != expected_header:
            errors.append(
                f"{repo_root / fixture_relative}: replay fixture must retain "
                "the exact version, seed, and column header"
            )
        rows = lines[3:]
        if len(rows) != 100:
            errors.append(
                f"{repo_root / fixture_relative}: replay fixture must contain "
                f"exactly 100 actions; found {len(rows)}"
            )
        parsed_rows = [row.split("\t") for row in rows]
        if any(
            len(columns) != 7
            or not columns[0].isdigit()
            or int(columns[0]) != index
            for index, columns in enumerate(parsed_rows, 1)
        ):
            errors.append(
                f"{repo_root / fixture_relative}: replay fixture rows must be "
                "contiguous seven-column actions"
            )
        if not parsed_rows or len(parsed_rows[-1]) != 7 or parsed_rows[-1][1] != (
            "PersistDecision"
        ):
            errors.append(
                f"{repo_root / fixture_relative}: replay fixture must end at "
                "PersistDecision"
            )

    normalizer_relative = "scripts/normalize_sumeragi_v2_tlc_trace.py"
    for fragment, description in (
        ("from typing import Union\n", "the Python 3.9-compatible union import"),
        (
            "Scalar = Union[int, str]\n",
            "the Python 3.9-compatible scalar alias",
        ),
        ("REPLAY_ACTIONS = frozenset(\n", "the closed replay-action vocabulary"),
        (
            'if actions[-1].action != "PersistDecision":\n',
            "the terminal PersistDecision check",
        ),
        (
            '"# sumeragi-v2-tlc-action-trace-v1",\n',
            "the fixture format marker",
        ),
        (
            "sys.stdout.write(render(actions, arguments.seed))\n",
            "the validated trace renderer",
        ),
    ):
        require_once(normalizer_relative, fragment, description)
    normalizer_source = sources.get(normalizer_relative)
    if normalizer_source is not None:
        for forbidden in ("from typing import TypeAlias", "Scalar: TypeAlias"):
            if forbidden in normalizer_source:
                errors.append(
                    f"{repo_root / normalizer_relative}: replay normalizer "
                    "must remain importable by the supported Python 3.9 runtime"
                )

    model_relative = "crates/iroha_sumeragi_core/tests/model_trace_replay.rs"
    for fragment, description in (
        (
            'const TRACE: &str = include_str!("fixtures/tlc_replay_witness.tsv");\n',
            "the checked-in fixture inclusion",
        ),
        (
            "fn tlc_liveness_witness_replays_against_the_production_reducer() {\n",
            "the production replay entry test",
        ),
        (
            'let steps = parse_trace(TRACE).expect("checked-in source-aligned '
            'trace is valid");\n',
            "the checked-in trace parser",
        ),
        ("assert_eq!(steps.len(), 100);\n", "the exact 100-action assertion"),
    ):
        require_once(model_relative, fragment, description)

    replay_relative = "scripts/formal/check_sumeragi_v2_replay_trace.sh"
    replay_fragments = (
        (
            'readonly EXPECTED="${FIXTURE_DIR}/tlc_replay_witness.tsv"\n',
            "the expected fixture path",
        ),
        (
            'readonly CONFIG="${FIXTURE_DIR}/tlc_replay_witness.cfg"\n',
            "the witness configuration path",
        ),
        (
            'readonly NORMALIZER="${REPO_ROOT}/scripts/'
            'normalize_sumeragi_v2_tlc_trace.py"\n',
            "the normalizer path",
        ),
        (
            'source "${REPO_ROOT}/scripts/formal/'
            'sumeragi_v2_tlc_result_contract.sh"\n',
            "the shared TLC result-contract import",
        ),
        (
            'if [[ "$tlc_status" -ne 12 || ! -s "$tlc_log" ]]; then\n',
            "the intentional TLC violation-status check",
        ),
        (
            'sumeragi_v2_tlc_assert_regular_log '
            '"replay-decision-witness" "$tlc_log"\n',
            "the fresh regular witness-log assertion",
        ),
        (
            'python3 "$NORMALIZER" "$tlc_log" --seed "$SEED" '
            '>"$normalized_trace"',
            "the pinned-seed normalizer invocation",
        ),
        (
            'if ! cmp -s "$EXPECTED" "$normalized_trace"; then\n',
            "the exact normalized-fixture comparison",
        ),
        (
            'bash "$REPO_ROOT/scripts/formal/run_sumeragi_v2_harness.sh" '
            "--model-replay\n",
            "the production replay dispatcher",
        ),
    )
    for fragment, description in replay_fragments:
        require_once(replay_relative, fragment, description)
    replay_source = sources.get(replay_relative)
    if replay_source is not None:
        ordered_fragments = (
            replay_fragments[3][0],
            "tlc2.TLC",
            "tlc_status=$?",
            replay_fragments[4][0],
            replay_fragments[5][0],
            replay_fragments[6][0],
            replay_fragments[7][0],
            replay_fragments[8][0],
        )
        positions = [replay_source.find(fragment) for fragment in ordered_fragments]
        if any(position < 0 for position in positions) or positions != sorted(
            positions
        ):
            errors.append(
                f"{repo_root / replay_relative}: replay gate must order the "
                "shared result contract, TLC, status and regular-log "
                "validation, normalization, exact comparison, and production "
                "dispatch"
            )

    harness_relative = "scripts/formal/run_sumeragi_v2_harness.sh"
    harness_source = sources.get(harness_relative)
    if harness_source is not None:
        inventory = re.search(
            r"(?ms)^    required_replay_tests=\(\n(?P<body>.*?)^    \)\n",
            harness_source,
        )
        if inventory is None:
            errors.append(
                f"{repo_root / harness_relative}: replay harness must declare "
                "the exact eight-test inventory"
            )
        else:
            observed_tests = tuple(
                line.strip()
                for line in inventory.group("body").splitlines()
                if line.strip()
            )
            if observed_tests != REPLAY_MODEL_TESTS:
                errors.append(
                    f"{repo_root / harness_relative}: replay harness must declare "
                    "the exact eight-test inventory in canonical order"
                )
        harness_fragments = (
            "--model-replay)",
            "model_replay_test_list=",
            'if ((${#listed_replay_tests[@]} != '
            '${#required_replay_tests[@]})); then',
            "replay_ignored_test_list=",
            "if ((${#listed_ignored_replay_tests[@]} != 0)); then",
            "--test model_trace_replay -- --test-threads=1",
        )
        positions = [harness_source.find(fragment) for fragment in harness_fragments]
        if any(position < 0 for position in positions) or positions != sorted(
            positions
        ):
            errors.append(
                f"{repo_root / harness_relative}: replay harness must inventory "
                "exactly eight runnable tests before executing the suite"
            )

    return errors


_EXACT_TLA_CALL_STATEMENT_TOKEN = re.compile(
    r"=>|\\[A-Za-z]+|[A-Za-z_][A-Za-z0-9_]*|[():,]"
)


def _exact_tla_call_statement_tokens(
    statement: str,
) -> tuple[str, ...] | None:
    """Tokenize one closed wrapper-call statement, ignoring layout only.

    Direct release obligations deliberately have a tiny reviewed grammar:
    one quantifier followed by nested operator calls, optionally with one
    implication between a reviewed premise and conclusion.  Token comparison
    makes line breaks and spaces around call punctuation immaterial without
    ever joining a split identifier, a split quantifier, or an inserted
    logical operator.  Returning ``None`` for every other token keeps this
    exact-source check fail-closed if the statement grows beyond that reviewed
    grammar.
    """

    tokens: list[str] = []
    index = 0
    while index < len(statement):
        if statement[index].isspace():
            index += 1
            continue
        match = _EXACT_TLA_CALL_STATEMENT_TOKEN.match(statement, index)
        if match is None:
            return None
        tokens.append(match.group(0))
        index = match.end()
    return tuple(tokens)


_SAME_ROUND_STRICT_TLA_SOURCE_SHA256 = {
    "SumeragiV2InductiveProofs.tla": (
        "74ed46cc0c891ee2b54b708bf025f1422e913a28108d6135ed1fb40286de6573"
    ),
    "SumeragiV2Proofs.tla": (
        "98d4af84143ef116ea96872bca81c6b79b606f5936718f69d6f28b7fda7b2ab2"
    ),
}


_READINESS_TLA_SOURCE_SHA256 = {
    "SumeragiV2BeginTimeoutReadyProofs.tla": (
        "65e8c168f83f67fd1f017ec6bc8b8acaf6cbb979f3a72dd3260534ea03ce513a"
    ),
    "SumeragiV2RegularCommandFramedReadyProofs.tla": (
        "e07164a45953103598501f29d2ed7279268d8b2edba1b301372a9633bae138df"
    ),
    "SumeragiV2RegularCommandExecutionReadyProofs.tla": (
        "758d3e233c9143ac6f60e85521d65d1d04d556ddda3aa0bda7da6ab269e783c1"
    ),
    "SumeragiV2NonRegularCommandExecutionReadyProofs.tla": (
        "b86cedd16802ce5f00f85a336749c5cf2c2a5d5b064ea54027eb5d93fc537507"
    ),
    "SumeragiV2CommandExecutionReadyProofs.tla": (
        "16927b07a9dfb5a45322f4d8a0de6daeeb7f009d551fc4dd56864208a11ddb35"
    ),
}


_READINESS_TOOL_SOURCE_SHA256 = {
    "ci/check_sumeragi_formal.sh": (
        "42ccb814bf79222dec698afa497871e4a8a9d7b4fe7e8a6b0395ce94245a09f3"
    ),
    "scripts/formal/check_sumeragi_v2_begin_timeout_ready_contract.py": (
        "70676b4b572c1b6cbd420ffbcc3f638a151fa3cb60d094d52c27556e6cfee4da"
    ),
    "scripts/formal/run_sumeragi_v2_begin_timeout_ready_mutation.sh": (
        "bb69d0d022c3111db49f6016220122c3617b2c701d5127ee73eb81140b981cbe"
    ),
    "scripts/formal/check_sumeragi_v2_command_execution_ready_contract.py": (
        "607f4f7e7f4b98706e175099a30da305a72d295df8ac1f6554cc914c69145b63"
    ),
    "scripts/formal/run_sumeragi_v2_command_execution_ready_mutation.sh": (
        "1507c6d2442df0cf7a804f20f2869321457f9d678829bcc0effad55e3deae64d"
    ),
}


def _readiness_contract_module(script_name: str) -> Any:
    """Load one source-fidelity checker from its sealed repository path."""

    path = ROOT_DIR / "scripts" / "formal" / script_name
    module_name = f"_sumeragi_v2_{path.stem}"
    spec = importlib.util.spec_from_file_location(module_name, path)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot load readiness source contract: {path}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def _readiness_kernel_source_fidelity_errors(
    formal_dir: Path, repo_root: Path = ROOT_DIR
) -> list[str]:
    """Seal pure scheduler readiness, both consumers, proofs, and mutations."""

    errors: list[str] = []
    for name, expected_sha256 in _READINESS_TLA_SOURCE_SHA256.items():
        path = formal_dir / name
        if not path.is_file() or path.is_symlink():
            errors.append(f"{path}: readiness proof source must be a regular file")
            continue
        observed_sha256 = hashlib.sha256(path.read_bytes()).hexdigest()
        if observed_sha256 != expected_sha256:
            errors.append(
                f"{path}: readiness proof source must match exact reviewed "
                f"SHA-256 {expected_sha256}; found {observed_sha256}"
            )

    for relative, expected_sha256 in _READINESS_TOOL_SOURCE_SHA256.items():
        path = repo_root / relative
        if not path.is_file() or path.is_symlink():
            errors.append(f"{path}: readiness gate source must be a regular file")
            continue
        observed_sha256 = hashlib.sha256(path.read_bytes()).hexdigest()
        if observed_sha256 != expected_sha256:
            errors.append(
                f"{path}: readiness gate source must match exact reviewed "
                f"SHA-256 {expected_sha256}; found {observed_sha256}"
            )

    core = formal_dir / "SumeragiV2Core.tla"
    network = formal_dir / "SumeragiV2AsyncNetwork.tla"
    begin_proof = formal_dir / "SumeragiV2BeginTimeoutReadyProofs.tla"
    command_proof = formal_dir / "SumeragiV2CommandExecutionReadyProofs.tla"
    try:
        begin_contract = _readiness_contract_module(
            "check_sumeragi_v2_begin_timeout_ready_contract.py"
        )
        errors.extend(begin_contract.validate(core, network, begin_proof))
    except (AttributeError, ImportError, OSError, RuntimeError, SyntaxError, ValueError) as error:
        errors.append(f"BeginTimeout readiness source contract failed to load: {error}")
    try:
        command_contract = _readiness_contract_module(
            "check_sumeragi_v2_command_execution_ready_contract.py"
        )
        errors.extend(command_contract.validate(network, command_proof))
    except (AttributeError, ImportError, OSError, RuntimeError, SyntaxError, ValueError) as error:
        errors.append(f"command readiness source contract failed to load: {error}")

    ci_path = repo_root / "ci" / "check_sumeragi_formal.sh"
    if ci_path.is_file() and not ci_path.is_symlink():
        ci_lines = ci_path.read_text(encoding="utf-8").splitlines()
        required_invocations = (
            "bash scripts/formal/run_sumeragi_v2_begin_timeout_ready_mutation.sh",
            "bash scripts/formal/run_sumeragi_v2_command_execution_ready_mutation.sh",
        )
        positions: list[int] = []
        for invocation in required_invocations:
            if ci_lines.count(invocation) != 1:
                errors.append(
                    f"{ci_path}: formal release must invoke {invocation!r} "
                    "exactly once"
                )
                continue
            positions.append(ci_lines.index(invocation))
        if len(positions) == len(required_invocations) and positions != sorted(positions):
            errors.append(
                f"{ci_path}: BeginTimeout readiness mutation must run before "
                "the command-execution readiness mutation"
            )
    return errors


def _same_round_strict_tla_source_fidelity_errors(
    formal_dir: Path,
) -> list[str]:
    """Seal the strict same-round theorem and its non-vacuous proof chain."""

    errors: list[str] = []
    sources: dict[str, tuple[Path, str]] = {}
    for name, expected_sha256 in _SAME_ROUND_STRICT_TLA_SOURCE_SHA256.items():
        path = formal_dir / name
        if not path.is_file() or path.is_symlink():
            errors.append(f"{path}: same-round TLA source must be a regular file")
            continue
        payload = path.read_bytes()
        observed_sha256 = hashlib.sha256(payload).hexdigest()
        if observed_sha256 != expected_sha256:
            errors.append(
                f"{path}: same-round TLA source must match exact reviewed "
                f"SHA-256 {expected_sha256}; found {observed_sha256}"
            )
        source = payload.decode("utf-8")
        sources[name] = (path, source)

    for name in ("SumeragiV2Core.tla", "SumeragiV2Inductive.tla"):
        path = formal_dir / name
        if not path.is_file() or path.is_symlink():
            errors.append(f"{path}: same-round TLA dependency must be a regular file")
            continue
        sources[name] = (path, path.read_text(encoding="utf-8"))

    theorem_contracts = {
        "SumeragiV2InductiveProofs.tla": {
            "PendingLockCommitUsesExactCurrentRound": (
                "PendingVoteWritesAuthorized => \\A request \\in "
                "pendingLockCommit: /\\ request.vote.view = "
                "nodeView[request.node] /\\ "
                "CurrentOpenPrepareForCommit(request.node, request.qc) "
                "BY SMT DEF PendingVoteWritesAuthorized, "
                "CurrentOpenPrepareForCommit"
            ),
            "ReducerProvenanceImpliesSameRoundLockAndCommitAuthorization": (
                "ReducerProvenanceInvariant => "
                "SameRoundLockAndCommitAuthorizationInvariant BY "
                "PendingLockCommitUsesExactCurrentRound, "
                "DurableTimeoutProtectionIsDirect DEF "
                "ReducerProvenanceInvariant, "
                "SameRoundLockAndCommitAuthorizationInvariant"
            ),
        },
        "SumeragiV2Proofs.tla": {
            "SameRoundLockAndCommitAuthorizationObligation": (
                "\\A initialContext: "
                "SameRoundLockAndCommitAuthorizationProperty( "
                "CoreSpecAt(initialContext)) PROOF <1>1. ASSUME NEW "
                "initialContext PROVE "
                "SameRoundLockAndCommitAuthorizationProperty( "
                "CoreSpecAt(initialContext)) <2>1. "
                "CoreSpecAt(initialContext) => []StrongInductiveInvariant "
                "BY CoreSpecAtAlwaysStrongInductiveInvariant <2>2. "
                "StrongInductiveInvariant => "
                "SameRoundLockAndCommitAuthorizationInvariant BY "
                "ReducerProvenanceImpliesSameRoundLockAndCommitAuthorization "
                "DEF StrongInductiveInvariant <2> QED BY <2>1, <2>2, PTL "
                "DEF SameRoundLockAndCommitAuthorizationProperty <1> QED BY <1>1"
            ),
        },
    }
    for name, contracts in theorem_contracts.items():
        path_source = sources.get(name)
        if path_source is None:
            continue
        path, source = path_source
        for symbol, expected in contracts.items():
            extracted = _top_level_theorem_body(
                source, symbol, preserve_string_contents=True
            )
            if extracted is None:
                errors.append(f"{path}: missing reviewed same-round theorem {symbol}")
                continue
            body, line = extracted
            normalized = " ".join(body.split())
            if normalized != expected:
                errors.append(
                    f"{path}:{line}: {symbol} must retain the exact reviewed "
                    f"same-round proof body {expected!r}; found {normalized!r}"
                )

    proof_path_source = sources.get("SumeragiV2Proofs.tla")
    if proof_path_source is not None:
        path, source = proof_path_source
        symbol = "SameRoundLockAndCommitAuthorizationProperty"
        extracted = _top_level_operator_body(source, symbol)
        expected = (
            "specification => "
            "[]SameRoundLockAndCommitAuthorizationInvariant"
        )
        if extracted is None:
            errors.append(f"{path}: missing reviewed same-round property {symbol}")
        else:
            body, line = extracted
            normalized = " ".join(body.split())
            if normalized != expected:
                errors.append(
                    f"{path}:{line}: {symbol} must equal only the exact "
                    f"non-vacuous property {expected!r}; found {normalized!r}"
                )

    operator_contracts = {
        "SumeragiV2Core.tla": {
            "CurrentOpenPrepareForCommit": (
                "/\\ QcAt(node, qc) \\in receivedQCs "
                "/\\ qc.view = nodeView[node] "
                "/\\ ~NodeTimedOut(node, qc.view)"
            ),
            "HistoricalLockedPrepareForCommit": "FALSE",
        },
        "SumeragiV2Inductive.tla": {
            "SameRoundLockAndCommitAuthorizationInvariant": (
                "/\\ \\A request \\in pendingLockCommit: "
                "/\\ request.vote.view = nodeView[request.node] "
                "/\\ CurrentOpenPrepareForCommit(request.node, request.qc) "
                "/\\ \\A timeoutVote \\in timeoutIntents, "
                "commitVote \\in commitIntents: "
                "(/\\ timeoutVote.signer \\in Honest "
                "/\\ commitVote.signer = timeoutVote.signer "
                "/\\ commitVote.context = timeoutVote.context "
                "/\\ commitVote.phase = \"Commit\" "
                "/\\ commitVote.view <= timeoutVote.view) "
                "=> TimeoutVoteStrictlyProtectsCommit(timeoutVote, commitVote)"
            ),
        },
    }
    for name, contracts in operator_contracts.items():
        path_source = sources.get(name)
        if path_source is None:
            continue
        path, source = path_source
        for symbol, expected in contracts.items():
            extracted = _top_level_operator_body(
                source, symbol, preserve_string_contents=True
            )
            if extracted is None:
                errors.append(
                    f"{path}: missing reviewed same-round dependency {symbol}"
                )
                continue
            body, line = extracted
            normalized = " ".join(body.split())
            if normalized != expected:
                errors.append(
                    f"{path}:{line}: {symbol} must equal only the exact "
                    f"reviewed same-round dependency {expected!r}; "
                    f"found {normalized!r}"
                )

    core_path_source = sources.get("SumeragiV2Core.tla")
    if core_path_source is not None:
        path, source = core_path_source
        action_contracts = {
            "BeginLockCommit": (
                "CurrentOpenPrepareForCommit(node, qc)",
                'Vote(context, qc.view, "Commit", qc.subject, node)',
                "pendingLockCommit' = pendingLockCommit \\cup {request}",
            ),
            "PersistLockCommit": (
                "commitIntents' = commitIntents \\cup {request.vote}",
                "signVotes' = signVotes \\cup {signRequest}",
            ),
        }
        for symbol, required in action_contracts.items():
            extracted = _top_level_operator_body(
                source, symbol, preserve_string_contents=True
            )
            if extracted is None:
                errors.append(f"{path}: missing reviewed same-round action {symbol}")
                continue
            body, line = extracted
            normalized = " ".join(body.split())
            missing = tuple(
                fragment for fragment in required if fragment not in normalized
            )
            if missing:
                errors.append(
                    f"{path}:{line}: {symbol} must retain the exact same-round "
                    f"authorization path; missing {missing!r}"
                )
            if (
                symbol == "BeginLockCommit"
                and "HistoricalLockedPrepareForCommit(node, qc)" in normalized
            ):
                errors.append(
                    f"{path}:{line}: BeginLockCommit may not restore historical "
                    "split-round Commit authorization"
                )
    return errors


def _atomic_timeout_completion_source_fidelity_errors(
    formal_dir: Path,
) -> list[str]:
    """Seal atomic local-timeout completion and every semantic projection."""

    errors: list[str] = []
    guard_symbol = "LocalTimeoutCompletionGuard"
    guard_invocation = "LocalTimeoutCompletionGuard(request)"
    exact_guard = (
        "LET node == request.node "
        "vote == request.vote "
        "IN /\\ request \\in signTimeouts "
        "/\\ node \\in Honest \\cap up \\cap CurrentVoters "
        "/\\ node \\notin PendingNodes "
        "/\\ NoDecisionForNode(node) "
        "/\\ vote = LocalTimeoutVoteFor(node) "
        "/\\ vote.context = context "
        "/\\ vote.height = height "
        "/\\ vote.view = nodeView[node] "
        "/\\ vote.signer = node "
        "/\\ vote.highestPrepareQc = highestPrepareQc[node] "
        "/\\ vote.highRank = highestRank[node] "
        "/\\ vote.highSubject = highestSubject[node] "
        "/\\ vote \\in timeoutIntents "
        "/\\ ExactPrepareQcMatchesRef( "
        "vote.highestPrepareQc, vote.highRank, vote.highSubject) "
        "/\\ vote.highRank <= vote.view"
    )
    exact_core_kernels = {
        "TimeoutVotesIn": (
            "{received.vote: "
            "received \\in {entry \\in receipts: "
            "/\\ entry.node = node "
            "/\\ entry.vote.context = context "
            "/\\ entry.vote.view = roundView}}"
        ),
        "TimeoutVotesAt": (
            "TimeoutVotesIn(receivedTimeoutVotes, node, roundView)"
        ),
        "TimeoutCertificateAfterReceipt": (
            "TC(context, vote.view, "
            "TimeoutVotesIn(TimeoutReceiptsAfter(node, vote), "
            "node, vote.view))"
        ),
        "TimeoutInstallRequestAfterReceipt": (
            "InstallTcWal(node, "
            "TimeoutCertificateAfterReceipt(node, vote), TRUE)"
        ),
        "TimeoutReceiptSurvivesInstall": (
            "LET installedView == "
            "IF StrictSameRoundTcUpgrade(node, tc) "
            "THEN nodeView[node] ELSE tc.view + 1 "
            "IN \\/ received.node # node "
            "\\/ /\\ received.vote.context = context "
            "/\\ received.vote.height = height "
            "/\\ received.vote.view >= installedView "
            "/\\ received.vote.view <= installedView + 1"
        ),
    }
    exact_async_install_kernels = {
        "CurrentTimeoutControlFor": (
            "LET installedView == "
            "IF StrictSameRoundTcUpgrade(node, tc) "
            "THEN nodeView[node] ELSE tc.view + 1 "
            'IN {item \\in RetainedClassItems(items, node, "TimeoutVote"): '
            "/\\ item.envelope.vote.context = context "
            "/\\ item.envelope.vote.height = height "
            "/\\ item.envelope.vote.view >= installedView "
            "/\\ item.envelope.vote.view <= installedView + 1 "
            "/\\ item.envelope.vote.signer = node}"
        ),
        "InstalledControlAfterTC": (
            "LET withoutOwnTc == retained \\ RetainedClassItems("
            'retained, node, "TimeoutCertificate") '
            "remembered == RememberedControl(withoutOwnTc, items) "
            "installed == {item \\in remembered: "
            "item.source # node \\/ ControlClass(item) "
            "\\in AsyncInstallRetainedControlKinds} "
            "withCurrentTimeout == installed \\cup "
            "CurrentTimeoutControlFor(remembered, node, tc) "
            "IN ReseedExactHighestPrepareControl("
            "withCurrentTimeout, node, tc)"
        ),
    }

    core_path = formal_dir / "SumeragiV2Core.tla"
    if core_path.is_file():
        core_source = core_path.read_text(encoding="utf-8")
        stripped_core = strip_tla_comments(
            core_source, preserve_string_contents=True
        )
        declarations = re.findall(
            rf"(?m)^{guard_symbol}\s*\([^)=]*\)\s*==",
            stripped_core,
        )
        guard = _top_level_operator_body(
            core_source, guard_symbol, preserve_string_contents=True
        )
        if len(declarations) != 1 or guard is None:
            errors.append(
                f"{core_path}: require exactly one top-level {guard_symbol} "
                f"operator; found {len(declarations)}"
            )
        else:
            body, line = guard
            normalized = " ".join(body.split())
            if normalized != exact_guard:
                errors.append(
                    f"{core_path}:{line}: {guard_symbol} must equal only the "
                    "exact reviewed local timeout completion "
                    f"ownership/current-vote contract {exact_guard!r}; "
                    f"found {normalized!r}"
                )

        for symbol, exact_body in exact_core_kernels.items():
            declarations = re.findall(
                rf"(?m)^{re.escape(symbol)}\s*\([^)=]*\)\s*==",
                stripped_core,
            )
            extracted = _top_level_operator_body(
                core_source, symbol, preserve_string_contents=True
            )
            if len(declarations) != 1 or extracted is None:
                errors.append(
                    f"{core_path}: require exactly one top-level {symbol} "
                    f"operator; found {len(declarations)}"
                )
                continue
            body, line = extracted
            normalized = " ".join(body.split())
            if normalized != exact_body:
                errors.append(
                    f"{core_path}:{line}: {symbol} must equal only the "
                    "exact reviewed atomic timeout receipt/certificate/install "
                    f"kernel {exact_body!r}; found {normalized!r}"
                )

        completion = _top_level_operator_body(
            core_source,
            "CompleteTimeoutSignature",
            preserve_string_contents=True,
        )
        if completion is None:
            errors.append(
                f"{core_path}: missing reviewed CompleteTimeoutSignature action"
            )
        else:
            body, line = completion
            observed_count = " ".join(body.split()).count(
                f"/\\ {guard_invocation}"
            )
            if observed_count != 1:
                errors.append(
                    f"{core_path}:{line}: CompleteTimeoutSignature must invoke "
                    f"{guard_invocation} exactly once; found {observed_count}"
                )

    network_path = formal_dir / "SumeragiV2AsyncNetwork.tla"
    if network_path.is_file():
        network_source = network_path.read_text(encoding="utf-8")
        stripped_network = strip_tla_comments(
            network_source, preserve_string_contents=True
        )
        ready_symbol = "CompleteTimeoutSignatureReady"
        declarations = re.findall(
            rf"(?m)^{ready_symbol}\s*\([^)=]*\)\s*==",
            stripped_network,
        )
        ready = _top_level_operator_body(
            network_source, ready_symbol, preserve_string_contents=True
        )
        if len(declarations) != 1 or ready is None:
            errors.append(
                f"{network_path}: require exactly one top-level {ready_symbol} "
                f"operator; found {len(declarations)}"
            )
        else:
            body, line = ready
            normalized = " ".join(body.split())
            if normalized != guard_invocation:
                errors.append(
                    f"{network_path}:{line}: {ready_symbol} must equal only "
                    f"{guard_invocation!r}; found {normalized!r}"
                )

        for symbol, exact_body in exact_async_install_kernels.items():
            declarations = re.findall(
                rf"(?m)^{re.escape(symbol)}\s*\([^)=]*\)\s*==",
                stripped_network,
            )
            extracted = _top_level_operator_body(
                network_source, symbol, preserve_string_contents=True
            )
            if len(declarations) != 1 or extracted is None:
                errors.append(
                    f"{network_path}: require exactly one top-level {symbol} "
                    f"operator; found {len(declarations)}"
                )
                continue
            body, line = extracted
            normalized = " ".join(body.split())
            if normalized != exact_body:
                errors.append(
                    f"{network_path}:{line}: {symbol} must equal only the "
                    "exact reviewed bounded timeout install-retention kernel "
                    f"{exact_body!r}; found {normalized!r}"
                )

    return errors


_INSTALLED_TC_SELECTOR_PROOF_SHA256 = (
    "99743a47d15918454ec638c35556ed1a7d985247d982ecd31032d7dfec0292bf"
)


_LOCAL_PROPOSAL_TIMEOUT_RUST_ITEM_SHA256 = {
    "refinement_macro": (
        "6d85f748f8d3b91f5f74371a2863222fcc85414993f619868eab4d1b431a3e26"
    ),
    "refinement_projection": (
        "bcbe07fa648cea2261d61dd61febf6d5551c1a92d002c484fddc24debc837d10"
    ),
    "refinement_projection_builder": (
        "f590ae87da7b3fc60758630ad8900d819611e5e8cc6c314096cb3904dcdcd692"
    ),
    "refinement_kernel": (
        "6dfd8e876f96b226b5f8959fe244c00816fffb9c967cffc459272325e01e649c"
    ),
    "wal_kernel_adapter": (
        "111c5d17aba435a3e2a9f593cb6cc4612df9752c0f5add4b7dea06a47f1b2983"
    ),
    "wal_replay": (
        "7473b0680ec743e30070bc5dcb5ca9d1c7934199852c861fb5e0e9796e7ab709"
    ),
    "reducer_active_proposal": (
        "dd2dd74ab7257442e5ce7017be1f4579d8716a162960a043763180a0f1016525"
    ),
    "reducer_local_proposal": (
        "99de499e1686f8e88376fa7a3a1542bfb5e7e9a413d1106e39de2601ad9aec85"
    ),
    "reducer_resume": (
        "5110f5c231d638f1b0ec7964f2fab3b91470cfeeae5a59ec320e45a478ca64cd"
    ),
    "reducer_replay_plan": (
        "4fa17c19bbfb517ff6991c57d35474030fe6d4f236d6f16593c5caceaba877cd"
    ),
    "test_live_exact": (
        "98f307efb1a6957c0652f45f4151627d32e054ebb34548c73a929b6037f4bfc2"
    ),
    "test_recovery_exact": (
        "a3ed4601ee6bd3ff42b4088daa2c9e1dea54f24f5b5297ff4e6706a4948a0616"
    ),
    "test_recovery_superseded": (
        "2b193232b9e943ad9529a68abb6d34421325de4319943f820c5b49a9164daba0"
    ),
    "verus_projection": (
        "0d67af3316c8d4a0f5f12a53d042c9a429aaea758d31aa872e823c0aa096d859"
    ),
    "verus_spec": (
        "5038efa6bed7bea498c758aa42e16e7531145b3fa9482a7e9d013283b1bc5473"
    ),
    "verus_executable_equivalence": (
        "0268669c50a1418d7a2141dac718fc22f29335dea6a453f64cf2bfdab77bca73"
    ),
    "verus_latest_binding": (
        "c31fd0daff8bf506b6b6f95aaa2daef2afd092d8c51a081e73a340d2d6fb2d14"
    ),
    "verus_foreign_rejection": (
        "47e0e5b14e40e57557e55ab222f40bf4761ead7ef555278096f41b1a16227c7c"
    ),
    "verus_wal_guard": (
        "f506842dcecdf438ed71b84afe7aabda51ead8f2b9dd04348f720a1c8b3bdf30"
    ),
    "verus_wal_consequence": (
        "5347c5222fccbd36dc1e519f7f8140d96aa9073984f221a403497495d8a42c81"
    ),
}


_PROPOSAL_TIMEOUT_EXACTNESS_RUST_ITEM_SHA256 = {
    "proposal_validate": (
        "8c2418e5ea37d1f40c540014be16ad687d6841b1ce7e5d4113570c2ee22c99a4"
    ),
    "justification_to_core": (
        "7cb05a8ea4751352cb45b4dfb7cc2b321f0d28382175a6e8576fa942dc8e1104"
    ),
    "proposal_is_safe_for_lock": (
        "f3acc9c98b9e00a25ba462409364972e8714da10fa836912a72811655559fd9f"
    ),
    "wire_regression": (
        "d4ad8d326dd83cd21f15261ef8ca648dfd8828cf0f62808a39bf5e4cf1f9eb01"
    ),
    "adapter_regression": (
        "da4be67ab88f7683c231afbde6ce90cb027057cb500f9fa8cb9c7d37178345db"
    ),
}


def _same_round_semantic_kernel_source_fidelity_errors(
    repo_root: Path = ROOT_DIR,
) -> list[str]:
    """Bind shared vote/Commit identities to every production and Verus caller."""

    errors: list[str] = []
    sources: dict[str, tuple[Path, str]] = {}
    for relative, expected_sha256 in (
        _SAME_ROUND_SEMANTIC_KERNEL_SOURCE_SHA256.items()
    ):
        path, source = _read_reviewed_rust_source(
            repo_root,
            relative,
            errors,
            "same-round semantic kernel source",
        )
        if not source:
            continue
        observed_sha256 = hashlib.sha256(source.encode("utf-8")).hexdigest()
        if observed_sha256 != expected_sha256:
            errors.append(
                f"{path}: same-round semantic kernel source must match exact "
                f"reviewed SHA-256 {expected_sha256}; found {observed_sha256}"
            )
        sources[relative] = (path, source)

    required_sequences = {
        "crates/iroha_core/src/sumeragi/v2_core/refinement.rs": (
            (
                "macro_rules! vote_statement_identity_equal_body",
                "production must define one shared signer-independent vote-statement kernel",
            ),
            (
                "macro_rules! certificate_height_subject_identity_equal_body",
                "production must define one shared semantic certificate-body kernel",
            ),
            (
                """
macro_rules! strict_same_round_timeout_upgrade_body {
    ($projection:expr, $zero:expr, $one:expr) => {{
        let projection = $projection;
        projection.current_view > $zero
            && projection.timeout_view == projection.current_view - $one
            && projection.installed_same_round
            && projection.selected_prepare_present
            && (!projection.highest_prepare_present
                || projection.selected_prepare_view > projection.highest_prepare_view)
            && (!projection.locked_prepare_present
                || projection.selected_prepare_view > projection.locked_prepare_view)
    }};
}
""",
                "production and Verus must share one nontrivial strict same-round timeout-upgrade predicate",
            ),
            (
                """
pub(crate) const fn strict_same_round_timeout_upgrade_is_allowed(
    projection: StrictSameRoundTimeoutUpgradeProjection,
) -> bool {
    let zero: u64 = 0;
    let one: u64 = 1;
    strict_same_round_timeout_upgrade_body!(projection, zero, one)
}
""",
                "the executable strict timeout-upgrade kernel must invoke the shared proof body",
            ),
            (
                """
&& timeout.view < u64::MAX
&& $projection.after_tag.view == timeout.view + 1u64
&& $projection.before_tag.view <= $projection.after_tag.view
""",
                "EnterView must consume the admitted monotonic post-view instead of transcribing a second strict-upgrade predicate",
            ),
            (
                """
fn transition_facts(projection: TransitionProjection<'_>) -> TransitionFacts {
    transition_facts_from_projection_body!(
        projection,
        TransitionFacts,
        TransitionClassificationFacts,
        TransitionDeltaFacts,
    )
}
""",
                "production must derive transition facts only from the shared projection constructor",
            ),
            (
                """
let timeout_vote_pool_unchanged =
    $projection.timeout_votes_before == $projection.timeout_votes_after;
let formed_timeouts_unchanged =
    $projection.formed_timeouts_before == $projection.formed_timeouts_after;
let timeout_evidence_after_in_installed_window =
    $projection.timeout_evidence_after_outside_installed_window == 0u64;
let timeout_control_unchanged =
    $projection.timeout_control_before == $projection.timeout_control_after;
let timeout_control_after_absent = $projection.timeout_control_after.is_none();
""",
                "production must derive timeout identity and the installed current/adjacent window from primitive projections",
            ),
            (
                """
if $facts.install_view_unchanged {
    $facts.timeout_vote_pool_unchanged
        && $facts.volatile_after.timeout_vote_pools
        == $facts.volatile_before.timeout_vote_pools
        && $facts.volatile_after.timeout_vote_entries
            == $facts.volatile_before.timeout_vote_entries
} else {
    $facts.timeout_evidence_after_in_installed_window
        && $facts.volatile_after.timeout_vote_pools
        <= $facts.volatile_before.timeout_vote_pools
        && $facts.volatile_after.timeout_vote_entries
            <= $facts.volatile_before.timeout_vote_entries
}
""",
                "the production gate must reject same-size substitution and require bounded non-inventing timeout retention",
            ),
            (
                """
if $facts.install_view_unchanged {
    $facts.formed_timeouts_unchanged
        && $facts.volatile_after.formed_timeouts
        == $facts.volatile_before.formed_timeouts
} else {
    $facts.volatile_after.formed_timeouts
        <= $facts.volatile_before.formed_timeouts
}
""",
                "the production gate must preserve exact same-round markers and forbid advancing-install invention",
            ),
            (
                """
if $facts.install_view_unchanged {
    $facts.timeout_control_unchanged
} else {
    $facts.timeout_control_after_absent
}
""",
                "the production gate must preserve exact timeout-control identity across a lock-only install and require absence after advance",
            ),
            (
                "accepts_facts(transition_facts(projection))",
                "the production commit gate must consume facts derived from the exact projection",
            ),
            (
                """
pub(crate) fn production_durable_intent_trace_refines_progress_witness_kernel(
    projection: ProductionDurableIntentTraceProjection,
) -> bool {
    production_durable_intent_trace_body!(projection)
}
""",
                "the durable-intent production kernel must invoke its shared proof body",
            ),
        ),
        "crates/iroha_core/src/sumeragi/v2_core/types.rs": (
            (
                """
pub fn same_statement(self, other: Self) -> bool {
    vote_statement_identity_equal_body!(
        self.context_id,
        self.round.height,
        self.round.view,
        self.proposal_round.height,
        self.proposal_round.view,
        self.phase,
        self.subject,
        other.context_id,
        other.round.height,
        other.round.view,
        other.proposal_round.height,
        other.proposal_round.view,
        other.phase,
        other.subject,
    )
}
""",
                "Vote::same_statement must invoke the shared identity kernel without signer",
            ),
            (
                """
pub fn same_height_subject(self, other: Self) -> bool {
    certificate_height_subject_identity_equal_body!(
        self.context_id,
        self.round.height,
        self.subject,
        other.context_id,
        other.round.height,
        other.subject,
    )
}
""",
                "CertificateRef::same_height_subject must invoke the shared semantic body kernel",
            ),
            (
                """
self.phase == Phase::Commit
    && other.phase == Phase::Commit
    && self.same_height_subject(other)
""",
                "same_commit_decision must require Commit phase and the shared semantic body",
            ),
        ),
        "crates/iroha_core/src/sumeragi/v2_core/reducer.rs": (
            (
                "vote.same_statement(intent)",
                "Commit vote admission must compare the exact signer-independent statement",
            ),
            (
                """
if let Some(existing) = self.durable.decision() {
    if !existing
        .reference()
        .same_commit_decision(certificate.reference())
""",
                "duplicate Commit admission must use stable semantic decision identity",
            ),
            (
                """
if let Some(existing) = self.durable.decision() {
    if !existing
        .reference()
        .same_commit_decision(certificate.reference())
    {
        return Err(ReducerError::ConflictingDecision);
    }
    return Ok(StepOutcome::ignored(IgnoreReason::Duplicate));
}
let effect = self.start_persistence(
""",
                "Commit admission must let the first validated CommitQC supersede any Prepare lock and reject only a conflicting durable Decision",
            ),
            (
                "carried.same_commit_decision(frozen)",
                "view-zero proposal admission must bind the semantic parent decision",
            ),
            (
                "let Some(checked_refinement) = refinement::check(transition) "
                "else { let diagnostic = refinement::diagnose(transition); "
                "iroha_logger::error!(event = ?audit_event, ?diagnostic, "
                '"Sumeragi v2 reducer rejected the transition refinement predicate"); '
                "return Err(ReducerError::RefinementViolation); };",
                "Reducer::step must acquire checked production refinement "
                "authority before committing candidate state",
            ),
            (
                """
let _authorized_refinement = checked_refinement.into_projection();
*self = next;
""",
                "Reducer::step must consume checked refinement authority before commit",
            ),
            (
                "let Some(checked_transition) = "
                "check_production_durable_intent_transition(durable_intent_trace) "
                "else { iroha_logger::error!(event = ?audit_event, "
                "?durable_intent_trace, "
                '"Sumeragi v2 reducer rejected the durable-intent refinement predicate"); '
                "return Err(ReducerError::RefinementViolation); };",
                "Reducer::step must acquire the source-shared durable-intent "
                "authorization token",
            ),
            (
                """
let _authorized_transition = checked_transition.into_projection();
let _authorized_refinement = checked_refinement.into_projection();
""",
                "Reducer::step must consume both checked transition authorities",
            ),
            (
                """
if certificate.round().view() < self.durable.current_view()
    && !self
        .durable
        .is_strict_same_round_timeout_upgrade(&certificate)
""",
                "live timeout-certificate admission must invoke the durable source-shared strict-upgrade adapter",
            ),
            (
                """
if self
    .durable
    .is_strict_same_round_timeout_upgrade(certificate)
{
    self.generation.next()
} else {
    Some(Generation::INITIAL)
}
""",
                "InstallTimeout generation must classify the exact strict same-round upgrade and reset only advancing views",
            ),
            (
                """
let next_generation = match pending.entry.record() {
    WalRecord::InstallTimeout(certificate) => self
        .generation_after_timeout_install(certificate)
        .ok_or(ReducerError::GenerationOverflow)?,
    _ => self.generation,
};
""",
                "InstallTimeout acknowledgement must preflight same-round generation exhaustion and advancing-view reset before durable mutation",
            ),
            (
                "self.generation = next_generation;",
                "InstallTimeout acknowledgement must commit only the preflighted next generation",
            ),
            (
                """
if expected
    .apply(&self.context, self.local_validator, &pending.entry)
    .is_err()
    || expected != after.durable
{
    return false;
}
""",
                "the acknowledgement refinement must re-run the source-shared durable WAL admission before accepting EnterView effects",
            ),
            (
                """
let current_view = self.durable.current_view();
self.timeout_votes.retain(|round, _| {
    round.height() == self.context.height()
        && timeout_vote_view_is_admissible(current_view, round.view())
});
self.formed_timeouts.retain(|round| {
    round.height() == self.context.height()
        && timeout_vote_view_is_admissible(current_view, round.view())
});
""",
                "InstallTimeout must retain only installed current/adjacent timeout evidence",
            ),
            (
                """
OutboundControlClass::CommitVote
    | OutboundControlClass::PrepareQc
    | OutboundControlClass::CommitQc
    | OutboundControlClass::TimeoutVote
    | OutboundControlClass::TimeoutCertificate
""",
                "strict same-round InstallTimeout must retain the active exact TimeoutVote and highest PrepareQC control owners",
            ),
            (
                """
let timeout_evidence_after_outside_installed_window = Self::cardinality(
    after
        .timeout_votes
        .keys()
        .chain(after.formed_timeouts.iter())
        .filter(|round| {
            round.height() != installed_height
                || !timeout_vote_view_is_admissible(installed_view, round.view())
        })
        .count(),
);
""",
                "the transition projection must count every timeout-evidence round outside the installed current/adjacent window",
            ),
            (
                """
formed_timeouts_after: &after.formed_timeouts,
timeout_evidence_after_outside_installed_window,
timeout_control_before: self
    .outbound_control
    .get(&OutboundControlClass::TimeoutVote),
timeout_control_after: after
    .outbound_control
    .get(&OutboundControlClass::TimeoutVote),
""",
                "the transition projection must preserve direct timeout-control key occupancy and full message identity",
            ),
            (
                """
fn same_round_timeout_upgrade_accepts_the_last_generation() {
""",
                "the final representable same-round InstallTimeout generation must remain covered by a positive regression",
            ),
            (
                """
fn same_round_timeout_generation_overflow_preserves_the_complete_state() {
""",
                "same-round generation overflow must retain a regression for complete reducer-state non-mutation",
            ),
            (
                """
let before = pending.clone();

let error = pending
    .step(event.clone())
    .expect_err("an exhausted generation must reject the install");

assert_eq!(error, ReducerError::GenerationOverflow);
assert_eq!(pending, before);
""",
                "the public reducer step must preserve every durable, pending, and volatile owner on generation overflow",
            ),
            (
                """
let Event::Persisted { id, .. } = event else {
    panic!("timeout-install fixture must return a persistence acknowledgement")
};
let mut in_place = before.clone();

let error = in_place
    .on_persisted(id)
    .expect_err("the in-place callback must precheck generation exhaustion");

assert_eq!(error, ReducerError::GenerationOverflow);
assert_eq!(in_place, before);
""",
                "the in-place acknowledgement callback must preserve the complete reducer state on generation overflow",
            ),
        ),
        "crates/iroha_core/src/sumeragi/v2_core/wal.rs": (
            (
                """
pub(crate) fn is_strict_same_round_timeout_upgrade(
    &self,
    certificate: &TimeoutCertificate,
) -> bool {
    let selected = certificate.highest_prepare();
    refinement::strict_same_round_timeout_upgrade_is_allowed(
        StrictSameRoundTimeoutUpgradeProjection {
            current_view: self.current_view,
            timeout_view: certificate.round().view(),
            installed_same_round: self
                .last_timeout
                .as_ref()
                .is_some_and(|installed| installed.round() == certificate.round()),
""",
                "the durable adapter must project exact installed-round identity into the shared strict-upgrade kernel",
            ),
            (
                """
let strict_same_round_upgrade =
    self.is_strict_same_round_timeout_upgrade(certificate);
""",
                "WAL replay must classify strict timeout upgrades only through the shared durable adapter",
            ),
            (
                "carried.same_commit_decision(frozen)",
                "WAL proposal replay must bind the semantic parent decision",
            ),
            (
                """
WalRecord::Decision(certificate) => {
    validate_qc(context, certificate, Phase::Commit)?;
    if let Some(existing) = &self.decision {
        if !existing
            .reference()
            .same_commit_decision(certificate.reference())
        {
            return Err(ReplayError::ConflictingDecision);
        }
    } else {
        self.decision = Some(certificate.clone());
    }
}
""",
                "WAL Decision replay must let the first validated CommitQC supersede any Prepare lock and reject only a conflicting durable Decision",
            ),
            (
                """
!existing
    .reference()
    .same_commit_decision(certificate.reference())
""",
                "WAL Decision replay must reject a conflicting semantic decision",
            ),
        ),
        "crates/iroha_core/src/sumeragi/v2_effects.rs": (
            (
                """
if decision_round.context_id != self.context.id()
    || decision_round.height != self.context.height
    || proposal_round.context_id != self.context.id()
    || proposal_round.height != self.context.height
    || proposal_round != decision_round
{
    return Err(EffectExecutorError::Contract(
        "durable Decision is outside the frozen height context".to_owned(),
    ));
}
match self.protected_decision {
""",
                "effect reconciliation must let the first durable Decision supersede any protected Prepare lock",
            ),
            (
                """
self.protected_decision = Some(durable_decision);
self.protected_lock = Some(decision_body);
self.decision_body_drained |= drain_decision_body;
""",
                "effect reconciliation must rebind terminal protection to the exact durable Decision",
            ),
        ),
        "crates/iroha_sumeragi_core/src/verus_proofs.rs": (
            (
                """
pub open spec fn strict_same_round_timeout_upgrade(
    before: WalStateProjection,
    tc_view: int,
    selected_prepare: CertificateProjection,
) -> bool {
    let zero: int = 0;
    let one: int = 1;
    strict_same_round_timeout_upgrade_body!(
        StrictSameRoundTimeoutUpgradeProjection {
            current_view: before.view,
            timeout_view: tc_view,
            installed_same_round: before.last_timeout_view == tc_view,
            selected_prepare_present: selected_prepare.present,
            selected_prepare_view: selected_prepare.view,
            highest_prepare_present: before.highest_prepare.present,
            highest_prepare_view: before.highest_prepare.view,
            locked_prepare_present: before.locked.present,
            locked_prepare_view: before.locked.view,
        },
        zero,
        one
    )
}
""",
                "Verus must instantiate the shared strict timeout-upgrade body from its primitive WAL projection",
            ),
            (
                """
pub open spec fn same_vote_statement(
    left: VoteStatementProjection,
    right: VoteStatementProjection,
) -> bool {
    vote_statement_identity_equal_body!(
""",
                "Verus vote identity must invoke the shared production macro",
            ),
            (
                """
pub open spec fn same_certificate_height_subject(
    left: CertificateProjection,
    right: CertificateProjection,
) -> bool {
    certificate_height_subject_identity_equal_body!(
""",
                "Verus certificate identity must invoke the shared production macro",
            ),
            (
                """
left.present
    && right.present
    && !left.prepare
    && !right.prepare
    && same_certificate_height_subject(left, right)
""",
                "Verus semantic Commit identity must retain nontrivial phase and body checks",
            ),
            (
                """
let delta_facts = verified_delta_facts_from_projection(projection);
let classification_facts =
    verified_classification_facts_from_projection(projection, delta_facts);
let enter_view_exact = verified_enter_view_fact_from_projection(projection);
let facts = transition_facts_from_components_body!(
""",
                "Verus must construct production facts from the three shared pure kernels",
            ),
            (
                """
pub timeout_evidence_after_outside_installed_window: u64,
""",
                "Verus must receive the exact production-projected outside-window evidence count",
            ),
            (
                """
let facts = transition_delta_facts_from_projection_body!(
    projection,
    ProductionTransitionDeltaFactsProjection
);
proof {
    reveal(production_delta_facts_from_projection);
}
facts
""",
                "Verus must prove the executable delta facts through the same timeout-owner projection used by production",
            ),
            (
                """
left.install_view_unchanged == right.install_view_unchanged
    && left.timeout_vote_pool_unchanged == right.timeout_vote_pool_unchanged
    && left.formed_timeouts_unchanged == right.formed_timeouts_unchanged
    && left.timeout_evidence_after_in_installed_window
        == right.timeout_evidence_after_in_installed_window
    && left.timeout_control_unchanged == right.timeout_control_unchanged
    && left.timeout_control_after_absent == right.timeout_control_after_absent
""",
                "Verus transition-fact extensionality must include every timeout-owner delta field",
            ),
            (
                """
facts.volatile_after.timeout_vote_pools <= 2,
facts.volatile_after.timeout_vote_entries <= facts.validator_count * 2,
facts.volatile_after.formed_certificates <= 2,
facts.volatile_after.formed_timeouts <= 2,
""",
                "Verus volatile bounds must cover exactly the current and adjacent timeout rounds",
            ),
            (
                """
if facts.install_view_unchanged {
    facts.timeout_vote_pool_unchanged
        && facts.volatile_after.timeout_vote_pools
            == facts.volatile_before.timeout_vote_pools
        && facts.volatile_after.timeout_vote_entries
            == facts.volatile_before.timeout_vote_entries
} else {
    facts.timeout_evidence_after_in_installed_window
        && facts.volatile_after.timeout_vote_pools
            <= facts.volatile_before.timeout_vote_pools
        && facts.volatile_after.timeout_vote_entries
            <= facts.volatile_before.timeout_vote_entries
}
""",
                "Verus volatile preservation must prove bounded non-inventing timeout retention on advancing installs",
            ),
            (
                """
if facts.install_view_unchanged {
    facts.formed_timeouts_unchanged
        && facts.volatile_after.formed_timeouts
            == facts.volatile_before.formed_timeouts
} else {
    facts.volatile_after.formed_timeouts
        <= facts.volatile_before.formed_timeouts
}
""",
                "Verus formed-timeout preservation must forbid advancing-install invention",
            ),
            (
                """
if facts.install_view_unchanged {
    facts.timeout_control_unchanged
} else {
    facts.timeout_control_after_absent
}
""",
                "Verus volatile preservation must prove exact timeout control retention or advancing-view absence",
            ),
            (
                "pub closed spec fn production_transition_action_relation("
                "facts: ProductionTransitionFactsProjection,) -> bool { "
                "production_transition_gate_body!(facts, "
                "production_volatile_summary_well_formed, "
                "production_named_action_relation, "
                "production_effect_trace_relation, "
                "production_transition_branch_relation,) }",
                "the Verus production relation must gate source-derived facts "
                "through the shared transition body",
            ),
        ),
    }
    for relative, sequences in required_sequences.items():
        path_source = sources.get(relative)
        if path_source is None:
            continue
        path, source = path_source
        for sequence, description in sequences:
            _require_rust_source_token_sequence(
                path, source, sequence, description, errors
            )
    return errors


def _installed_tc_selector_source_fidelity_errors(
    formal_dir: Path,
) -> list[str]:
    """Bind the exact last-installed-TC selector, proof, and callers."""

    errors: list[str] = []
    sources: dict[str, tuple[Path, str]] = {}
    for name in (
        "SumeragiV2Core.tla",
        "SumeragiV2Inductive.tla",
        "SumeragiV2AsyncNetwork.tla",
        "SumeragiV2InstalledTcSelectorProofs.tla",
    ):
        path = formal_dir / name
        if not path.is_file() or path.is_symlink():
            errors.append(
                f"{path}: installed-TC selector source must be a regular file"
            )
            continue
        try:
            sources[name] = (path, path.read_text(encoding="utf-8"))
        except (OSError, UnicodeDecodeError) as error:
            errors.append(f"{path}: cannot read installed-TC selector source: {error}")

    operator_contracts = {
        "SumeragiV2Core.tla": {
            "ProposalJustified": (
                "\\/ /\\ proposal.view = 0 "
                "/\\ proposal.timeoutCertificate = NoTimeoutCertificate "
                "/\\ proposal.highestPrepareQc = NoPrepareQC "
                "/\\ proposal.justifyRank = NoRank "
                "/\\ proposal.justifySubject = context.parent "
                "\\/ /\\ proposal.view > 0 "
                "/\\ lastInstalledTc[node] # NoTimeoutCertificate "
                "/\\ [node |-> node, tc |-> lastInstalledTc[node]] "
                "\\in installedTCs "
                "/\\ lastInstalledTc[node].context = context "
                "/\\ lastInstalledTc[node].view + 1 = proposal.view "
                "/\\ TCValid(lastInstalledTc[node]) "
                "/\\ proposal.timeoutCertificate = lastInstalledTc[node] "
                "/\\ proposal.highestPrepareQc = "
                "lastInstalledTc[node].highestPrepareQc "
                "/\\ proposal.justifyRank = "
                "PrepareQcRank(proposal.highestPrepareQc) "
                "/\\ proposal.justifySubject = "
                "PrepareQcSubject(proposal.highestPrepareQc) "
                "/\\ proposal.justifyRank < proposal.view"
            ),
            "LocalProposalJustification": (
                "LET roundView == nodeView[node] "
                "IN IF roundView = 0 "
                "THEN [timeoutCertificate |-> NoTimeoutCertificate, "
                "highestPrepareQc |-> NoPrepareQC] "
                "ELSE LET tc == lastInstalledTc[node] "
                "IN [timeoutCertificate |-> tc, "
                "highestPrepareQc |-> tc.highestPrepareQc]"
            ),
            "LocalProposalFor": (
                "LET justification == LocalProposalJustification(node) "
                "IN Proposal(context, nodeView[node], subject, node, "
                "justification.timeoutCertificate, "
                "justification.highestPrepareQc)"
            ),
        },
        "SumeragiV2AsyncNetwork.tla": {
            "RestartLastInstalledTCs": (
                "IF lastInstalledTc[node] = NoTimeoutCertificate "
                "THEN {} ELSE {lastInstalledTc[node]}"
            ),
        },
        "SumeragiV2InstalledTcSelectorProofs.tla": {
            "LastInstalledTcEntry": (
                "[node |-> node, tc |-> lastInstalledTc[node]]"
            ),
            "ExactCurrentInstalledTcEntries": (
                "{installed \\in installedTCs: "
                "/\\ installed = LastInstalledTcEntry(node) "
                "/\\ lastInstalledTc[node] # NoTimeoutCertificate "
                "/\\ lastInstalledTc[node].context = context "
                "/\\ lastInstalledTc[node].view + 1 = roundView}"
            ),
            "ExactSelectedInstalledTcForRound": (
                "LastInstalledTcEntry(node)"
            ),
            "InstalledTcExactSelectionInvariant": (
                "\\A node \\in ValidatorIds: "
                "lastInstalledTc[node] # NoTimeoutCertificate "
                "=> /\\ LastInstalledTcEntry(node) \\in installedTCs "
                "/\\ TcWellTyped(lastInstalledTc[node]) "
                "/\\ lastInstalledTc[node].highestPrepareQc "
                "\\in PrepareQcOptionSet"
            ),
            "StrongInstalledTcExactSelectionInvariant": (
                "/\\ StrongInductiveInvariant "
                "/\\ InstalledTcExactSelectionInvariant"
            ),
        },
    }
    for name, contracts in operator_contracts.items():
        path_source = sources.get(name)
        if path_source is None:
            continue
        path, source = path_source
        stripped = strip_tla_comments(source, preserve_string_contents=True)
        for symbol, expected in contracts.items():
            declarations = re.findall(
                rf"(?m)^{re.escape(symbol)}\s*(?:\([^)=]*\))?\s*==",
                stripped,
            )
            extracted = _top_level_operator_body(
                source, symbol, preserve_string_contents=True
            )
            if len(declarations) != 1 or extracted is None:
                errors.append(
                    f"{path}: require exactly one top-level installed-TC "
                    f"selector operator {symbol}; found {len(declarations)}"
                )
                continue
            body, line = extracted
            observed = " ".join(body.split())
            if observed != expected:
                errors.append(
                    f"{path}:{line}: {symbol} must retain the exact installed-TC "
                    f"full-certificate identity or direct selector call path "
                    f"{expected!r}; found {observed!r}"
                )

    retired_symbols = (
        "InstalledTcAtLeastAsRecent",
        "CurrentInstalledTcEntries",
        "LatestCurrentInstalledTcEntries",
        "SelectedInstalledTCForRound",
        "InstalledTcCurrentFrontier",
        "InstalledTcEqualOrderKeyUnique",
        "InstalledTcSelectorShapeInvariant",
        "StrongInstalledTcSelectorInvariant",
        "LatestCurrentInstalledTcSelectorIsUnique",
        "BeginLocalProposalUsesUniqueLatestTcAndPreservesLock",
    )
    for name, (path, source) in sources.items():
        stripped = strip_tla_comments(source, preserve_string_contents=True)
        present = tuple(
            symbol
            for symbol in retired_symbols
            if re.search(rf"\b{re.escape(symbol)}\b", stripped)
        )
        if present:
            errors.append(
                f"{path}: retired history/rank installed-TC selector symbols "
                f"are prohibited; found {present!r}"
            )
        if name == "SumeragiV2InstalledTcSelectorProofs.tla":
            prohibited_tokens = tuple(
                token
                for token in ("CHOOSE", "TcHighRank", "TcHighSubject")
                if re.search(rf"\b{re.escape(token)}\b", stripped)
            )
            if prohibited_tokens:
                errors.append(
                    f"{path}: exact installed-TC proof may not reconstruct or "
                    "tie-break complete certificate identity; found "
                    f"{prohibited_tokens!r}"
                )

    proof_path_source = sources.get("SumeragiV2InstalledTcSelectorProofs.tla")
    if proof_path_source is not None:
        proof_path, proof_source = proof_path_source
        observed_sha256 = hashlib.sha256(proof_source.encode("utf-8")).hexdigest()
        if observed_sha256 != _INSTALLED_TC_SELECTOR_PROOF_SHA256:
            errors.append(
                f"{proof_path}: installed-TC selector proof source must match "
                "the exact reviewed SHA-256 "
                f"{_INSTALLED_TC_SELECTOR_PROOF_SHA256}; found {observed_sha256}"
            )
        if not proof_source.startswith(
            "---- MODULE SumeragiV2InstalledTcSelectorProofs ----\n"
            "EXTENDS SumeragiV2Proofs, FunctionTheorems\n"
        ):
            errors.append(
                f"{proof_path}: installed-TC selector proof must extend the "
                "reviewed Core proof and finite-set theorem modules"
            )

        theorem_statements = {
            "StrongInductiveInvariantProjectsExactInstalledTcSelection": (
                "StrongInductiveInvariant "
                "=> InstalledTcExactSelectionInvariant"
            ),
            "CoreSpecAtAlwaysStrongInstalledTcExactSelectionInvariant": (
                "\\A initialContext: CoreSpecAt(initialContext) "
                "=> []StrongInstalledTcExactSelectionInvariant"
            ),
            "ExactInstalledTcSelectorIsUnique": (
                "\\A node, roundView: "
                "(/\\ StrongInstalledTcExactSelectionInvariant "
                "/\\ ExactCurrentInstalledTcEntries(node, roundView) # {}) "
                "=> LET selected == "
                "ExactSelectedInstalledTcForRound(node, roundView) "
                "current == "
                "ExactCurrentInstalledTcEntries(node, roundView) "
                "IN /\\ selected \\in current "
                "/\\ \\A other \\in current: other = selected "
                "/\\ selected.tc = lastInstalledTc[node] "
                "/\\ selected.tc.highestPrepareQc = "
                "lastInstalledTc[node].highestPrepareQc"
            ),
            "BeginLocalProposalUsesExactInstalledTcAndPreservesLock": (
                "\\A node, subject: "
                "(/\\ StrongInstalledTcExactSelectionInvariant "
                "/\\ BeginLocalProposal(node, subject) "
                "/\\ nodeView[node] > 0) "
                "=> LET roundView == nodeView[node] "
                "selected == "
                "ExactSelectedInstalledTcForRound(node, roundView) "
                "current == "
                "ExactCurrentInstalledTcEntries(node, roundView) "
                "proposal == LocalProposalFor(node, subject) "
                "IN /\\ selected \\in current "
                "/\\ \\A other \\in current: other = selected "
                "/\\ proposal.timeoutCertificate = selected.tc "
                "/\\ proposal.highestPrepareQc = "
                "selected.tc.highestPrepareQc "
                "/\\ proposal.justifyRank = "
                "PrepareQcRank(selected.tc.highestPrepareQc) "
                "/\\ proposal.justifySubject = "
                "PrepareQcSubject(selected.tc.highestPrepareQc) "
                "/\\ (lockRank[node] # NoRank "
                "=> \\/ proposal.subject = lockSubject[node] "
                "\\/ /\\ selected.tc.highestPrepareQc # NoPrepareQC "
                "/\\ selected.tc.highestPrepareQc.view > lockRank[node] "
                "/\\ selected.tc.highestPrepareQc.subject = proposal.subject)"
            ),
        }
        required_dependencies = {
            "StrongInductiveInvariantProjectsExactInstalledTcSelection": (
                "StrongInductiveInvariant",
                "InstalledTcExactSelectionInvariant",
                "TypeInvariant",
                "LastInstalledTcEntry",
            ),
            "CoreSpecAtAlwaysStrongInstalledTcExactSelectionInvariant": (
                "CoreSpecAtAlwaysStrongInductiveInvariant",
                "StrongInductiveInvariantProjectsExactInstalledTcSelection",
                "StrongInstalledTcExactSelectionInvariant",
            ),
            "ExactInstalledTcSelectorIsUnique": (
                "InstalledTcExactSelectionInvariant",
                "ExactSelectedInstalledTcForRound",
                "ExactCurrentInstalledTcEntries",
                "LastInstalledTcEntry",
                "lastInstalledTc",
            ),
            "BeginLocalProposalUsesExactInstalledTcAndPreservesLock": (
                "ExactInstalledTcSelectorIsUnique",
                "ExactSelectedInstalledTcForRound",
                "ExactCurrentInstalledTcEntries",
                "LocalProposalJustification",
                "SafeToPrepare",
            ),
        }
        for symbol, expected_statement in theorem_statements.items():
            extracted = _top_level_theorem_body(
                proof_source, symbol, preserve_string_contents=True
            )
            if extracted is None:
                errors.append(
                    f"{proof_path}: missing non-vacuous installed-TC selector "
                    f"theorem {symbol}"
                )
                continue
            body, line = extracted
            parts = re.split(
                r"(?m)^[ \t]*(?:BY|PROOF|OBVIOUS)\b", body, maxsplit=1
            )
            observed_statement = " ".join(parts[0].split())
            if observed_statement != expected_statement:
                errors.append(
                    f"{proof_path}:{line}: {symbol} must retain its exact "
                    "nontrivial selector uniqueness/proposal postcondition; "
                    f"found {observed_statement!r}"
                )
            normalized_body = " ".join(body.split())
            missing = tuple(
                dependency
                for dependency in required_dependencies[symbol]
                if normalized_body.count(dependency) == 0
            )
            if missing:
                errors.append(
                    f"{proof_path}:{line}: {symbol} is disconnected from "
                    f"reviewed selector dependencies {missing!r}"
                )

    module_count = RELEASE_PROOF_MODULES.count(
        "SumeragiV2InstalledTcSelectorProofs"
    )
    if module_count != 1:
        errors.append(
            "release proof inventory must execute "
            "SumeragiV2InstalledTcSelectorProofs exactly once; "
            f"found {module_count}"
        )
    return errors


def _local_proposal_timeout_source_fidelity_errors(
    repo_root: Path = ROOT_DIR,
) -> list[str]:
    """Bind one exact durable timeout to proposal, replay, and Verus paths."""

    errors: list[str] = []
    sources: dict[str, tuple[Path, str]] = {}
    for relative in (
        "crates/iroha_core/src/sumeragi/v2_core/refinement.rs",
        "crates/iroha_core/src/sumeragi/v2_core/wal.rs",
        "crates/iroha_core/src/sumeragi/v2_core/reducer.rs",
        "crates/iroha_core/src/sumeragi/v2_core/tests.rs",
        "crates/iroha_sumeragi_core/src/verus_proofs.rs",
    ):
        path, source = _read_reviewed_rust_source(
            repo_root,
            relative,
            errors,
            "exact local-proposal timeout source",
        )
        if source:
            sources[relative] = (path, source)

    specs = (
        (
            "refinement_macro",
            "crates/iroha_core/src/sumeragi/v2_core/refinement.rs",
            "local_proposal_timeout_justification_body",
            (),
            (),
            "macro",
            "source-shared exact timeout-justification predicate",
        ),
        (
            "refinement_projection",
            "crates/iroha_core/src/sumeragi/v2_core/refinement.rs",
            "LocalProposalTimeoutJustificationProjection",
            (),
            ("#[derive(Clone, Copy, Debug, PartialEq, Eq)]",),
            "struct",
            "production exact timeout-justification projection",
        ),
        (
            "refinement_projection_builder",
            "crates/iroha_core/src/sumeragi/v2_core/refinement.rs",
            "local_proposal_timeout_projection",
            (),
            (),
            "item",
            "production exact timeout-justification projection builder",
        ),
        (
            "refinement_kernel",
            "crates/iroha_core/src/sumeragi/v2_core/refinement.rs",
            "local_proposal_timeout_justification_is_exact",
            (),
            ("#[must_use]",),
            "item",
            "production exact timeout-justification kernel",
        ),
        (
            "wal_kernel_adapter",
            "crates/iroha_core/src/sumeragi/v2_core/wal.rs",
            "is_exact_local_proposal_timeout_justification",
            (("impl", "DurableState"),),
            (),
            "item",
            "durable-state exact timeout adapter",
        ),
        (
            "wal_replay",
            "crates/iroha_core/src/sumeragi/v2_core/wal.rs",
            "apply_in_place",
            (("impl", "DurableState"),),
            ("#[allow(clippy::too_many_lines)]",),
            "item",
            "ProposalIntent WAL replay gate",
        ),
        (
            "reducer_active_proposal",
            "crates/iroha_core/src/sumeragi/v2_core/reducer.rs",
            "durable_proposal_is_active",
            (("impl", "Reducer"),),
            (),
            "item",
            "durable proposal retransmission authorization",
        ),
        (
            "reducer_local_proposal",
            "crates/iroha_core/src/sumeragi/v2_core/reducer.rs",
            "on_local_proposal_ready",
            (("impl", "Reducer"),),
            (),
            "item",
            "live local-proposal construction",
        ),
        (
            "reducer_resume",
            "crates/iroha_core/src/sumeragi/v2_core/reducer.rs",
            "on_resume_after_replay",
            (("impl", "Reducer"),),
            (),
            "item",
            "WAL replay resumption filter",
        ),
        (
            "reducer_replay_plan",
            "crates/iroha_core/src/sumeragi/v2_core/reducer.rs",
            "expected_replay_signatures",
            (("impl", "Reducer"),),
            (),
            "item",
            "independent WAL replay signature plan",
        ),
        (
            "test_live_exact",
            "crates/iroha_core/src/sumeragi/v2_core/tests.rs",
            "same_round_timeout_upgrade_is_exact_local_proposal_justification",
            (),
            ("#[test]",),
            "item",
            "live exact-timeout regression",
        ),
        (
            "test_recovery_exact",
            "crates/iroha_core/src/sumeragi/v2_core/tests.rs",
            "recovery_uses_same_round_timeout_upgrade_as_exact_local_proposal_justification",
            (),
            ("#[test]",),
            "item",
            "exact-timeout WAL recovery regression",
        ),
        (
            "test_recovery_superseded",
            "crates/iroha_core/src/sumeragi/v2_core/tests.rs",
            "recovery_excludes_proposal_intent_superseded_by_same_round_timeout_upgrade",
            (),
            ("#[test]",),
            "item",
            "superseded ProposalIntent recovery regression",
        ),
        (
            "verus_projection",
            "crates/iroha_sumeragi_core/src/verus_proofs.rs",
            "LocalProposalTimeoutJustificationProjection",
            (("verus", "!"),),
            (),
            "struct",
            "Verus exact timeout-justification projection",
        ),
        (
            "verus_spec",
            "crates/iroha_sumeragi_core/src/verus_proofs.rs",
            "local_proposal_timeout_justification_is_exact",
            (("verus", "!"),),
            (),
            "item",
            "Verus shared exact timeout specification",
        ),
        (
            "verus_executable_equivalence",
            "crates/iroha_sumeragi_core/src/verus_proofs.rs",
            "verified_local_proposal_timeout_justification_is_exact",
            (("verus", "!"),),
            (),
            "item",
            "Verus executable-kernel equivalence theorem",
        ),
        (
            "verus_latest_binding",
            "crates/iroha_sumeragi_core/src/verus_proofs.rs",
            "exact_local_proposal_timeout_justification_binds_latest_durable_tc",
            (("verus", "!"),),
            (),
            "item",
            "Verus latest-durable-timeout consequence theorem",
        ),
        (
            "verus_foreign_rejection",
            "crates/iroha_sumeragi_core/src/verus_proofs.rs",
            "foreign_local_proposal_timeout_evidence_is_rejected",
            (("verus", "!"),),
            (),
            "item",
            "Verus foreign-timeout-evidence rejection theorem",
        ),
        (
            "verus_wal_guard",
            "crates/iroha_sumeragi_core/src/verus_proofs.rs",
            "wal_frame_admissible",
            (("verus", "!"),),
            (),
            "item",
            "Verus ProposalIntent WAL guard",
        ),
        (
            "verus_wal_consequence",
            "crates/iroha_sumeragi_core/src/verus_proofs.rs",
            "proposal_intent_guard_binds_exact_latest_timeout",
            (("verus", "!"),),
            (),
            "item",
            "Verus ProposalIntent latest-timeout theorem",
        ),
    )
    observed_items: dict[str, tuple[Path, RustItem | None]] = {}
    for (
        key,
        relative,
        item_name,
        context,
        attributes,
        kind,
        description,
    ) in specs:
        path_source = sources.get(relative)
        if path_source is None:
            continue
        path, source = path_source
        if kind == "macro":
            matches = rust_macro_items(source, item_name)
            if len(matches) != 1:
                errors.append(
                    f"{path}: require exactly one real Rust macro item named "
                    f"{item_name}; found {len(matches)}"
                )
                item = None
            else:
                item = matches[0]
        elif kind == "struct":
            matches = rust_struct_items(source, item_name)
            if len(matches) != 1:
                errors.append(
                    f"{path}: require exactly one real Rust/Verus struct item "
                    f"named {item_name}; found {len(matches)}"
                )
                item = None
            else:
                item = matches[0]
        else:
            item = _require_rust_item(path, source, item_name, errors)
        _require_rust_item_context(
            path,
            item,
            context,
            description,
            errors,
            expected_attributes=attributes,
        )
        _require_rust_item_token_sha256(
            path,
            item,
            _LOCAL_PROPOSAL_TIMEOUT_RUST_ITEM_SHA256[key],
            description,
            errors,
        )
        observed_items[key] = (path, item)

    def require_sequence(key: str, sequence: str, description: str) -> None:
        path_item = observed_items.get(key)
        if path_item is None:
            return
        path, item = path_item
        _require_rust_token_sequence(
            path, item, sequence, description, errors
        )

    require_sequence(
        "refinement_macro",
        """
projection.current_view > $zero
    && projection.proposal_view == projection.current_view
    && projection.proposal_timeout_context_id == projection.expected_context_id
    && projection.durable_timeout_context_id == projection.expected_context_id
    && projection.proposal_timeout_height == projection.expected_height
    && projection.durable_timeout_height == projection.expected_height
    && projection.proposal_timeout_view == projection.current_view - $one
    && projection.durable_timeout_view == projection.current_view - $one
    && projection.proposal_timeout_group_count > $zero
    && projection.proposal_timeout_group_count == projection.durable_timeout_group_count
""",
        "shared timeout kernel must bind current/predecessor view, context, height, and groups",
    )
    require_sequence(
        "refinement_macro",
        """
projection.proposal_timeout_evidence_identity != $absent_evidence
    && projection.proposal_timeout_evidence_identity
        == projection.durable_timeout_evidence_identity
""",
        "shared timeout kernel must bind non-absent full certificate evidence identity",
    )
    require_sequence(
        "refinement_projection_builder",
        """
proposal_timeout_evidence_identity: if proposal_timeout == durable_timeout {
    LOCAL_PROPOSAL_TIMEOUT_EVIDENCE_MATCHED
} else {
    LOCAL_PROPOSAL_TIMEOUT_EVIDENCE_FOREIGN
}
""",
        "production projection must derive full timeout equality from concrete certificates",
    )
    require_sequence(
        "refinement_kernel",
        """
let Some(durable_timeout) = durable_timeout else {
    return false;
};
let projection = local_proposal_timeout_projection(
    expected_context_id,
    expected_height,
    current_view,
    proposal_view,
    proposal_timeout,
    durable_timeout,
);
""",
        "production kernel must build its own exact projection from the latest durable certificate",
    )
    require_sequence(
        "refinement_kernel",
        """
local_proposal_timeout_justification_body!(
    projection,
    zero,
    one,
    absent_evidence
)
""",
        "production kernel must return only the source-shared predicate",
    )
    require_sequence(
        "wal_kernel_adapter",
        """
refinement::local_proposal_timeout_justification_is_exact(
    self.context_id,
    self.height,
    self.current_view,
    proposal_view,
    certificate,
    self.last_timeout.as_ref(),
)
""",
        "durable-state adapter must supply the actual latest WAL timeout",
    )
    require_sequence(
        "wal_replay",
        """
ProposalJustification::Timeout(certificate)
    if proposal.round().view() > 0
        && certificate.validate(context).is_ok()
        && self.is_exact_local_proposal_timeout_justification(
            proposal.round().view(),
            certificate,
        )
""",
        "WAL ProposalIntent replay must reject a stale or foreign timeout justification",
    )
    require_sequence(
        "reducer_local_proposal",
        """
if !self
    .durable
    .is_exact_local_proposal_timeout_justification(round.view(), certificate)
{
    return Err(ReducerError::InvalidProposalJustification);
}
ProposalJustification::Timeout(certificate.clone())
""",
        "live local proposal construction must consume the exact durable timeout",
    )
    require_sequence(
        "reducer_active_proposal",
        """
ProposalJustification::Timeout(certificate) => durable
    .is_exact_local_proposal_timeout_justification(
        proposal.round().view(),
        certificate,
    )
""",
        "proposal signing/retransmission authorization must recheck the exact durable timeout",
    )
    for key in ("reducer_resume", "reducer_replay_plan"):
        require_sequence(
            key,
            """
.proposal_intent(round)
.filter(|proposal| Self::durable_proposal_is_active(&self.durable, proposal))
""",
            "WAL replay must not re-sign a ProposalIntent superseded by a later same-round TC",
        )
    require_sequence(
        "verus_spec",
        """
local_proposal_timeout_justification_body!(
    projection,
    zero,
    one,
    absent_evidence
)
""",
        "Verus specification must instantiate the source-shared production predicate",
    )
    require_sequence(
        "verus_executable_equivalence",
        """
ensures
    accepted == local_proposal_timeout_justification_is_exact(projection),
""",
        "Verus executable theorem must expose a nontrivial exact-result postcondition",
    )
    require_sequence(
        "verus_executable_equivalence",
        """
let accepted = local_proposal_timeout_justification_body!(
    projection,
    zero,
    one,
    absent_evidence
);
reveal(local_proposal_timeout_justification_is_exact);
accepted
""",
        "Verus executable theorem must prove the same macro result instead of a constant",
    )
    require_sequence(
        "verus_latest_binding",
        """
requires
    local_proposal_timeout_justification_is_exact(projection),
ensures
    projection.current_view > 0,
    projection.proposal_view == projection.current_view,
    projection.proposal_timeout_view == projection.current_view - 1,
    projection.durable_timeout_view == projection.current_view - 1,
""",
        "Verus latest-timeout theorem must retain exact parameters, precondition, and predecessor postconditions",
    )
    require_sequence(
        "verus_latest_binding",
        """
projection.proposal_timeout_evidence_identity
    == projection.durable_timeout_evidence_identity,
projection.proposal_timeout_evidence_identity != 0,
""",
        "Verus latest-timeout theorem must expose non-absent full evidence equality",
    )
    require_sequence(
        "verus_foreign_rejection",
        """
requires
    projection.proposal_timeout_evidence_identity
        != projection.durable_timeout_evidence_identity
        || projection.proposal_timeout_evidence_identity == 0,
ensures
    !local_proposal_timeout_justification_is_exact(projection),
""",
        "Verus foreign-evidence theorem must reject inequality or absent evidence",
    )
    require_sequence(
        "verus_wal_guard",
        """
&& local_proposal_timeout_justification_is_exact(
    timeout_justification,
)
""",
        "Verus WAL guard must invoke the exact timeout kernel",
    )
    verus_path_source = sources.get(
        "crates/iroha_sumeragi_core/src/verus_proofs.rs"
    )
    if verus_path_source is not None:
        verus_path, verus_source = verus_path_source
        _require_rust_source_token_sequence(
            verus_path,
            verus_source,
            """
if view > 0 {
    exact_local_proposal_timeout_justification_binds_latest_durable_tc(
        timeout_justification,
    );
}
""",
            "Verus WAL consequence must call the nontrivial latest-timeout theorem",
            errors,
        )
    return errors


def _proposal_timeout_exactness_source_fidelity_errors(
    repo_root: Path = ROOT_DIR,
) -> list[str]:
    """Bind TC selection to one complete repeated PrepareQC at every seam."""

    errors: list[str] = []
    sources: dict[str, tuple[Path, str]] = {}
    for relative in (
        "crates/iroha_data_model/src/block/consensus_v2.rs",
        "crates/iroha_core/src/sumeragi/v2.rs",
    ):
        path = repo_root / relative
        if not path.is_file() or path.is_symlink():
            errors.append(
                f"{path}: proposal timeout exactness source must be a regular file"
            )
            continue
        try:
            sources[relative] = (path, path.read_text(encoding="utf-8"))
        except (OSError, UnicodeDecodeError) as error:
            errors.append(
                f"{path}: cannot read proposal timeout exactness source: {error}"
            )

    data_model = sources.get(
        "crates/iroha_data_model/src/block/consensus_v2.rs"
    )
    core = sources.get("crates/iroha_core/src/sumeragi/v2.rs")
    observed: dict[str, tuple[Path, RustItem | None]] = {}

    if data_model is not None:
        path, source = data_model
        proposal_validate = _require_qualified_rust_item(
            path,
            source,
            "Proposal",
            "validate",
            errors,
            "Proposal::validate exact repeated-PrepareQC gate",
        )
        observed["proposal_validate"] = (path, proposal_validate)

        wire_regression = _require_rust_item(
            path,
            source,
            "timeout_proposal_accepts_only_the_selected_prepare_subject",
            errors,
        )
        _require_rust_item_context(
            path,
            wire_regression,
            (("#", "[", "cfg", "(", "test", ")", "]", "mod", "tests"),),
            "wire proposal exact timeout-evidence regression",
            errors,
            expected_attributes=("#[test]",),
        )
        observed["wire_regression"] = (path, wire_regression)

    if core is not None:
        path, source = core
        justification_to_core = _require_qualified_rust_item(
            path,
            source,
            "WireRegistry",
            "justification_to_core",
            errors,
            "WireRegistry::justification_to_core exact repeated-PrepareQC gate",
        )
        observed["justification_to_core"] = (path, justification_to_core)

        safe_for_lock = _require_rust_item(
            path, source, "proposal_is_safe_for_lock", errors
        )
        _require_rust_item_context(
            path,
            safe_for_lock,
            (),
            "proposal_is_safe_for_lock exact repeated-PrepareQC gate",
            errors,
        )
        observed["proposal_is_safe_for_lock"] = (path, safe_for_lock)

        adapter_regression = _require_rust_item(
            path,
            source,
            "locked_subject_reproposal_and_strict_higher_prepare_are_safe",
            errors,
        )
        _require_rust_item_context(
            path,
            adapter_regression,
            (("#", "[", "cfg", "(", "test", ")", "]", "mod", "tests"),),
            "adapter exact timeout-evidence regression",
            errors,
            expected_attributes=("#[test]",),
        )
        observed["adapter_regression"] = (path, adapter_regression)

    for key, (path, item) in observed.items():
        _require_rust_item_token_sha256(
            path,
            item,
            _PROPOSAL_TIMEOUT_EXACTNESS_RUST_ITEM_SHA256[key],
            {
                "proposal_validate": (
                    "Proposal::validate exact repeated-PrepareQC gate"
                ),
                "justification_to_core": (
                    "WireRegistry::justification_to_core exact "
                    "repeated-PrepareQC gate"
                ),
                "proposal_is_safe_for_lock": (
                    "proposal_is_safe_for_lock exact repeated-PrepareQC gate"
                ),
                "wire_regression": (
                    "wire proposal exact timeout-evidence regression"
                ),
                "adapter_regression": (
                    "adapter exact timeout-evidence regression"
                ),
            }[key],
            errors,
        )

    def require_sequence(
        key: str, sequence: str, description: str
    ) -> None:
        path_item = observed.get(key)
        if path_item is None:
            return
        path, item = path_item
        _require_rust_token_sequence(
            path, item, sequence, description, errors
        )

    require_sequence(
        "proposal_validate",
        """
let selected_highest = timeout.timeout_certificate.highest_prepare_qc();
if selected_highest != timeout.highest_prepare_qc.as_ref()
    || selected_highest.is_some_and(|highest| highest.subject != self.subject)
{
    return Err(ValidationError::InvalidProposalJustification);
}
""",
        "Proposal::validate must compare the complete TC-selected and repeated PrepareQC",
    )
    require_sequence(
        "justification_to_core",
        """
let selected = timeout.timeout_certificate.highest_prepare_qc();
if selected != timeout.highest_prepare_qc.as_ref() {
    return Err(AdapterError::InvalidProposalJustification);
}
let certificate = self.tc_to_core(&timeout.timeout_certificate, context)?;
""",
        "WireRegistry must reject unequal complete PrepareQC evidence before registry mutation",
    )
    require_sequence(
        "proposal_is_safe_for_lock",
        """
timeout.highest_prepare_qc.as_ref().is_some_and(|highest| {
    highest.phase == wire::GlobalPhase::Prepare
        && highest.round.context_id == locked_round.context_id
        && highest.round.height == locked_round.height
        && highest.round.view > locked_round.view
        && highest.subject == proposal.subject
        && timeout
            .timeout_certificate
            .highest_prepare_qc()
            .is_some_and(|selected| selected == highest)
})
""",
        "safe-value admission must compare the complete selected PrepareQC",
    )

    require_sequence(
        "wire_regression",
        """
proposal.justification = ProposalJustification::Timeout(TimeoutJustification {
    timeout_certificate: TimeoutCertificate {
        round: timeout_round,
        groups: vec![TimeoutVoteGroup {
            highest_prepare_qc: None,
            signers: vec![0, 1, 2],
            aggregate_signature: vec![0x43; 48],
        }],
    },
    highest_prepare_qc: Some(highest_prepare.clone()),
});
assert_eq!(
    proposal.validate(&context),
    Err(ValidationError::InvalidProposalJustification),
    "a proposal cannot invent a repeated high absent from its TC"
);
""",
        "wire regression must reject an invented repeated PrepareQC",
    )
    require_sequence(
        "wire_regression",
        """
proposal.justification = ProposalJustification::Timeout(TimeoutJustification {
    timeout_certificate: timeout_certificate.clone(),
    highest_prepare_qc: None,
});
assert_eq!(
    proposal.validate(&context),
    Err(ValidationError::InvalidProposalJustification),
    "a proposal cannot omit the exact high selected by its TC"
);
""",
        "wire regression must reject an omitted repeated PrepareQC",
    )
    require_sequence(
        "wire_regression",
        """
assert_eq!(alternate_evidence.as_ref(), highest_prepare.as_ref());
assert_ne!(alternate_evidence, highest_prepare);
proposal.justification = ProposalJustification::Timeout(TimeoutJustification {
    timeout_certificate,
    highest_prepare_qc: Some(alternate_evidence),
});
assert_eq!(
    proposal.validate(&context),
    Err(ValidationError::InvalidProposalJustification),
    "the repeated high must preserve the TC-selected full evidence"
);
""",
        "wire regression must reject same-reference alternate PrepareQC evidence",
    )

    for fixture, case_description in (
        ("missing_repeated_high", "omitted"),
        ("invented_repeated_high", "invented"),
        ("alternate_evidence", "same-reference alternate"),
    ):
        require_sequence(
            "adapter_regression",
            f"""
!proposal_is_safe_for_lock(&{fixture}, locked_round, locked_subject)
""",
            f"adapter regression must reject {case_description} evidence at safe-value admission",
        )
        require_sequence(
            "adapter_regression",
            f"""
.justification_to_core(&{fixture}.justification, &context),
Err(AdapterError::InvalidProposalJustification)
""",
            f"adapter regression must reject {case_description} evidence at wire conversion",
        )
    for registry, case_description in (
        ("missing_registry", "omitted"),
        ("invented_registry", "invented"),
        ("alternate_registry", "same-reference alternate"),
    ):
        require_sequence(
            "adapter_regression",
            f"""
{registry}.subjects.is_empty()
    && {registry}.execution_commitments.is_empty()
    && {registry}.certificates.is_empty()
""",
            f"adapter regression must prove {case_description} rejection precedes registry mutation",
        )

    return errors


_LOCKED_ASSEMBLY_RETENTION_MUTATION_SOURCE_SHA256 = {
    "SumeragiV2LockedAssemblyRetentionMutation.tla": (
        "5eb8308e5c3c717f60fb3c15bb12581fbd70793b543504f5922f7b92f728dce7"
    ),
    "locked_assembly_retention_old.cfg": (
        "9e089774ebf7b597a61da8fa5b2a1cea97cc2742e837238b7e97dd1a09169721"
    ),
    "locked_assembly_retention_fixed.cfg": (
        "f407cffec72c9c2e4f1efc52b1dd3e8e64eb0a438785c8e65a91554926368720"
    ),
}


def _locked_assembly_retention_mutation_errors(
    formal_dir: Path, repo_root: Path = ROOT_DIR
) -> list[str]:
    """Seal the Busy locked-assembly dispatch repair and its red/green TLC pair."""

    errors: list[str] = []
    for name, expected_sha256 in (
        _LOCKED_ASSEMBLY_RETENTION_MUTATION_SOURCE_SHA256.items()
    ):
        path = formal_dir / name
        if not path.is_file() or path.is_symlink():
            errors.append(
                f"{path}: locked-assembly retention source must be a regular file"
            )
            continue
        observed_sha256 = hashlib.sha256(path.read_bytes()).hexdigest()
        if observed_sha256 != expected_sha256:
            errors.append(
                f"{path}: locked-assembly retention source must match exact "
                f"reviewed SHA-256 {expected_sha256}; found {observed_sha256}"
            )

    async_path = formal_dir / "SumeragiV2AsyncNetwork.tla"
    if not async_path.is_file() or async_path.is_symlink():
        errors.append(
            f"{async_path}: locked-assembly dispatch source must be a regular file"
        )
    else:
        async_source = async_path.read_text(encoding="utf-8")
        exact_operators = {
            "LocalAssemblyBusyDispatchAllowed": (
                '/\\ command.class = "Normal" '
                '/\\ command.kind = "AssembleBody" '
                "/\\ command.item = NoAsyncItem"
            ),
            "CommandDispatchable": (
                "/\\ AsyncCandidateTyped(command) "
                "/\\ CandidateConsumerCurrent(command) "
                "/\\ CommandExecutionReady(command) "
                "/\\ (NodeIdle(command.node) "
                '\\/ command.class = "Completion" '
                "\\/ LocalAssemblyBusyDispatchAllowed(command))"
            ),
        }
        for symbol, expected in exact_operators.items():
            extracted = _top_level_operator_body(
                async_source, symbol, preserve_string_contents=True
            )
            if extracted is None:
                errors.append(
                    f"{async_path}: missing locked-assembly dispatch operator "
                    f"{symbol}"
                )
                continue
            body, line = extracted
            observed = " ".join(body.split())
            if observed != expected:
                errors.append(
                    f"{async_path}:{line}: {symbol} must equal the exact "
                    f"locked-assembly Busy dispatch contract {expected!r}; "
                    f"found {observed!r}"
                )

    runner = (
        repo_root / "scripts" / "formal" / "run_sumeragi_v2_progress_mutations.sh"
    )
    if not runner.is_file() or runner.is_symlink():
        errors.append(f"{runner}: missing locked-assembly retention mutation runner")
        return errors
    normalized_runner = re.sub(
        r"[ \t]*\\\r?\n[ \t]*", " ", runner.read_text(encoding="utf-8")
    )
    cases = (
        (
            "locked-assembly-retention-old",
            "SumeragiV2LockedAssemblyRetentionMutation.tla",
            "locked_assembly_retention_old.cfg",
            "12",
            (
                "Invariant AssemblyOwnershipPreserved is violated.",
                "State 2: <DispatchRuntimeHead",
                "2 states generated, 2 distinct states found, 0 states left on queue.",
            ),
        ),
        (
            "locked-assembly-retention-fixed",
            "SumeragiV2LockedAssemblyRetentionMutation.tla",
            "locked_assembly_retention_fixed.cfg",
            "0",
            (
                "Model checking completed. No error has been found.",
                "2 states generated, 2 distinct states found, 0 states left on queue.",
                "depth of the complete state graph search is 2",
            ),
        ),
    )
    offsets: list[int] = []
    for label, model, config, expected_status, markers in cases:
        pattern = re.compile(
            rf"run_case\s+{re.escape(label)}\s+"
            rf"{re.escape(model)}\s+"
            rf"{re.escape(config)}\s+{expected_status}\s+"
            + r'(?P<markers>(?:"[^"\n]*"\s*)+)'
        )
        matches = list(pattern.finditer(normalized_runner))
        if len(matches) != 1:
            errors.append(
                f"{runner}: locked-assembly mutation runner must invoke {label} "
                f"exactly once with status {expected_status}"
            )
            continue
        match = matches[0]
        offsets.append(match.start())
        observed_markers = tuple(
            re.findall(r'"([^"\n]*)"', match.group("markers"))
        )
        if observed_markers != markers:
            errors.append(
                f"{runner}: {label} must require exact markers {markers!r}; "
                f"found {observed_markers!r}"
            )
    if len(offsets) == len(cases) and offsets != sorted(offsets):
        errors.append(
            f"{runner}: locked-assembly retention cases must keep old-before-fixed "
            "order"
        )
    return errors


_LOCKED_BODY_REPROPOSAL_MUTATION_SOURCE_SHA256 = {
    "SumeragiV2LockedBodyReproposalMutation.tla": (
        "b371912c01cffeb971508d399f6bb0ace8f9f07331eacef80263affb50e570f5"
    ),
    "locked_body_reproposal_high_fixed.cfg": (
        "caf30d66fc61c372845689481baadb62c59d63956f77a02239ec3e67cbd6aa0a"
    ),
    "locked_body_reproposal_conflict_fixed.cfg": (
        "fe5ed92c6ce7b0463a98b42a9d0845e81a880a766096c8cfc54b78ba3edaaeae"
    ),
    "locked_body_reproposal_no_high_bug.cfg": (
        "5efb6a2e4e40a0cd42c23e186f4d1cbc43e116e45a5ee78bd581e1dd5a3f0e30"
    ),
    "locked_body_reproposal_equal_rank_bug.cfg": (
        "db75327f474d5eda933a700a73f20bd5de2ac01d6c2118e732a3eaf8dab2c603"
    ),
    "SumeragiV2HistoricalLockedRecoveryMutation.tla": (
        "f7653fa28bdf20633cc9aff8a9c972dd64b6f53505ae26e4fa6f05159a2c0e72"
    ),
    "historical_locked_recovery_fixed.cfg": (
        "af32274e3e634ad8ecd5f4524cde6e4151b957b22d58a22f801c2075a0435a1f"
    ),
    "historical_locked_recovery_installed_only.cfg": (
        "bc006a375f848b92609749f07134b529902baff0c5019936438bc15a7ed5b696"
    ),
    "historical_locked_recovery_fresh_commit_bug.cfg": (
        "e7a2f39c78f688dc66ce93f4968cf0d478e147cf4cef0e48f4b9369997adb9f9"
    ),
}


def _locked_body_reproposal_mutation_runner_errors(
    formal_dir: Path, repo_root: Path = ROOT_DIR
) -> list[str]:
    """Pin the strict same-round recovery/reproposal TLC mutation matrix."""

    errors: list[str] = []
    for name, expected_sha256 in (
        _LOCKED_BODY_REPROPOSAL_MUTATION_SOURCE_SHA256.items()
    ):
        path = formal_dir / name
        if not path.is_file() or path.is_symlink():
            errors.append(
                f"{path}: locked-body mutation source must be a regular file"
            )
            continue
        observed_sha256 = hashlib.sha256(path.read_bytes()).hexdigest()
        if observed_sha256 != expected_sha256:
            errors.append(
                f"{path}: locked-body mutation source must match exact reviewed "
                f"SHA-256 {expected_sha256}; found {observed_sha256}"
            )

    runner = (
        repo_root / "scripts" / "formal" / "run_sumeragi_v2_progress_mutations.sh"
    )
    if not runner.is_file() or runner.is_symlink():
        errors.append(f"{runner}: missing locked-body mutation runner")
        return errors
    normalized_runner = re.sub(
        r"[ \t]*\\\r?\n[ \t]*", " ", runner.read_text(encoding="utf-8")
    )
    cases = (
        (
            "locked-body-reproposal-high-fixed",
            "SumeragiV2LockedBodyReproposalMutation.tla",
            "locked_body_reproposal_high_fixed.cfg",
            "0",
            (
                "Model checking completed. No error has been found.",
                "4 states generated, 2 distinct states found, 0 states left on queue.",
            ),
        ),
        (
            "locked-body-reproposal-conflict-fixed",
            "SumeragiV2LockedBodyReproposalMutation.tla",
            "locked_body_reproposal_conflict_fixed.cfg",
            "0",
            (
                "Model checking completed. No error has been found.",
                "4 states generated, 2 distinct states found, 0 states left on queue.",
            ),
        ),
        (
            "locked-body-reproposal-no-high-bug",
            "SumeragiV2LockedBodyReproposalMutation.tla",
            "locked_body_reproposal_no_high_bug.cfg",
            "12",
            (
                "Invariant LaterHighReproposalAccepted is violated.",
                "2 states generated, 2 distinct states found, 0 states left on queue.",
            ),
        ),
        (
            "locked-body-reproposal-equal-rank-bug",
            "SumeragiV2LockedBodyReproposalMutation.tla",
            "locked_body_reproposal_equal_rank_bug.cfg",
            "12",
            (
                "Invariant EqualRankConflictRejected is violated.",
                "2 states generated, 2 distinct states found, 0 states left on queue.",
            ),
        ),
        (
            "historical-locked-recovery-fixed",
            "SumeragiV2HistoricalLockedRecoveryMutation.tla",
            "historical_locked_recovery_fixed.cfg",
            "0",
            (
                "Model checking completed. No error has been found.",
                "6 states generated, 3 distinct states found, 0 states left on queue.",
            ),
        ),
        (
            "historical-locked-recovery-installed-only-bug",
            "SumeragiV2HistoricalLockedRecoveryMutation.tla",
            "historical_locked_recovery_installed_only.cfg",
            "12",
            (
                "Invariant RecoveryOwnedAfterNoHighCarry is violated.",
                "4 states generated, 3 distinct states found, 0 states left on queue.",
            ),
        ),
        (
            "historical-locked-recovery-fresh-commit-bug",
            "SumeragiV2HistoricalLockedRecoveryMutation.tla",
            "historical_locked_recovery_fresh_commit_bug.cfg",
            "12",
            (
                "Invariant ExactIntentDoesNotAuthorizeFreshCommit is violated.",
                "4 states generated, 3 distinct states found, 0 states left on queue.",
            ),
        ),
    )
    offsets: list[int] = []
    for label, model, config, expected_status, markers in cases:
        pattern = re.compile(
            rf"run_case\s+{re.escape(label)}\s+"
            rf"{re.escape(model)}\s+"
            rf"{re.escape(config)}\s+{expected_status}\s+"
            + r'(?P<markers>(?:"[^"\n]*"\s*)+)'
        )
        matches = list(pattern.finditer(normalized_runner))
        if len(matches) != 1:
            errors.append(
                f"{runner}: locked-body mutation runner must invoke {label} "
                f"exactly once with status {expected_status}"
            )
            continue
        match = matches[0]
        offsets.append(match.start())
        observed_markers = tuple(
            re.findall(r'"([^"\n]*)"', match.group("markers"))
        )
        if observed_markers != markers:
            errors.append(
                f"{runner}: {label} must require exact markers {markers!r}; "
                f"found {observed_markers!r}"
            )
    if len(offsets) == len(cases) and offsets != sorted(offsets):
        errors.append(
            f"{runner}: locked-body reproposal and historical recovery cases "
            "must retain reviewed order"
        )
    return errors
