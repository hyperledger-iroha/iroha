# Executed lexically in check_sumeragi_v2_proof_ledger.py; do not import directly.

def _successor_stale_token_mutation_source_fidelity_errors(
    formal_dir: Path,
) -> list[str]:
    """Pin the two-state stale-token successor-start mutation witness."""
    model_path = formal_dir / "SumeragiV2SuccessorStaleTokenMutation.tla"
    bug_cfg_path = formal_dir / "successor_stale_token_bug.cfg"
    fixed_cfg_path = formal_dir / "successor_stale_token_fixed.cfg"
    errors: list[str] = []
    for path in (model_path, bug_cfg_path, fixed_cfg_path):
        if not path.is_file() or path.is_symlink():
            errors.append(
                f"{path}: missing required successor stale-token mutation artifact"
            )
    if not model_path.is_file() or model_path.is_symlink():
        return errors
    source = model_path.read_text(encoding="utf-8")
    def require_operator(
        symbol: str,
        *,
        required: tuple[str, ...] = (),
        forbidden: tuple[str, ...] = (),
        exact: str | None = None,
    ) -> None:
        extracted = _top_level_operator_body(
            source, symbol, preserve_string_contents=True
        )
        if extracted is None:
            errors.append(f"{model_path}: missing mutation operator {symbol}")
            return
        body, line = extracted
        normalized = " ".join(body.split())
        if exact is not None and normalized != exact:
            errors.append(
                f"{model_path}:{line}: mutation operator {symbol} must equal "
                f"only {exact!r}; found {normalized!r}"
            )
        missing = [token for token in required if token not in normalized]
        if missing:
            errors.append(
                f"{model_path}:{line}: mutation operator {symbol} omits "
                f"required stale-token behavior {missing}"
            )
        present = [token for token in forbidden if token in normalized]
        if present:
            errors.append(
                f"{model_path}:{line}: mutation operator {symbol} contains "
                f"prohibited repaired behavior {present}"
            )
    require_operator(
        "AppliedSuccessorActivationToken",
        exact=(
            '[kind |-> "Applied", parentContext |-> "Parent", '
            'node |-> "Node", successorContext |-> "Successor"]'
        ),
    )
    require_operator(
        "ExactDurableParentApplicationWitness",
        exact="TRUE",
    )
    require_operator(
        "SuccessorActivationPipelineDistance",
        required=(
            'CASE activationStatus = "Queued" -> 10',
            '/\\ activationStatus = "Running" /\\ '
            "~SuccessorActivationCredentialReady -> 9",
            "/\\ SuccessorActivationCredentialReady /\\ "
            "activationPrerequisites = {} -> 8",
            "[] OTHER -> 0",
        ),
    )
    require_operator(
        "SuccessorActivationRank",
        exact="SuccessorActivationPipelineDistance",
    )
    require_operator(
        "MutationTypeInvariant",
        required=(
            "activationPrerequisites \\subseteq "
            "SuccessorActivationRequiredPrerequisites",
            "activationTokens \\subseteq {AppliedSuccessorActivationToken}",
            'lastTransition \\in {"Initial", "BuggyBegin", "FixedBegin", '
            '"FixedReject", "AppliedFailure"}',
            "previousRank \\in 0..10",
        ),
    )
    require_operator(
        "SuccessorActivationProtocolInvariantProjection",
        exact=(
            "/\\ MutationTypeInvariant "
            "/\\ ExactDurableParentApplicationWitness "
            "/\\ (activationFailurePresent => "
            'activationStatus = "Running") '
            "/\\ SuccessorActivationPipelineDistance \\in 1..10"
        ),
    )
    require_operator(
        "StaleAppliedTokenState",
        exact=(
            '/\\ activationStatus = "Queued" '
            '/\\ predecessorOwnership = "Published" '
            '/\\ activationPrerequisites = {"IngressOpen"} '
            "/\\ activationTokens = {AppliedSuccessorActivationToken} "
            "/\\ activationFailurePresent = FALSE "
            "/\\ activationFailureHistoryPresent = FALSE"
        ),
    )
    require_operator(
        "StaleAppliedTokenInit",
        exact=(
            '/\\ StaleAppliedTokenState /\\ lastTransition = "Initial" '
            "/\\ previousRank = 10"
        ),
    )
    require_operator(
        "BuggyBeginSuccessorActivation",
        required=(
            'activationStatus = "Queued"',
            'predecessorOwnership = "Published"',
            "ExactDurableParentApplicationWitness",
            'activationStatus\' = "Running"',
            'lastTransition\' = "BuggyBegin"',
            "previousRank' = SuccessorActivationRank",
        ),
        forbidden=(
            "activationPrerequisites = {}",
            "AppliedSuccessorActivationToken \\notin activationTokens",
        ),
    )
    require_operator(
        "FixedBeginSuccessorActivation",
        required=(
            'activationStatus = "Queued"',
            'predecessorOwnership = "Published"',
            "ExactDurableParentApplicationWitness",
            "activationPrerequisites = {}",
            "AppliedSuccessorActivationToken \\notin activationTokens",
            'activationStatus\' = "Running"',
        ),
    )
    require_operator(
        "FixedRejectStaleSuccessorActivation",
        exact=(
            '/\\ StaleAppliedTokenState '
            '/\\ lastTransition = "Initial" '
            '/\\ lastTransition\' = "FixedReject" '
            "/\\ previousRank' = SuccessorActivationRank "
            "/\\ UNCHANGED <<activationStatus, predecessorOwnership, "
            "activationPrerequisites, activationTokens, "
            "activationFailurePresent, activationFailureHistoryPresent>>"
        ),
    )
    require_operator(
        "MutationLatchAppliedSuccessorStartupFailure",
        required=(
            'activationStatus = "Running"',
            'predecessorOwnership = "Published"',
            "~activationFailurePresent",
            "activationPrerequisites' = {}",
            "activationTokens' = {}",
            "activationFailurePresent' = TRUE",
            "activationFailureHistoryPresent' = TRUE",
            'lastTransition\' = "AppliedFailure"',
            "previousRank' = SuccessorActivationRank",
            "UNCHANGED <<activationStatus, predecessorOwnership>>",
        ),
    )
    exact_operators = {
        "StaleBuggyBeginIsEnabled": (
            "StaleAppliedTokenState => ENABLED "
            "BuggyBeginSuccessorActivation"
        ),
        "StaleFixedBeginIsDisabled": (
            "StaleAppliedTokenState => ~ENABLED "
            "FixedBeginSuccessorActivation"
        ),
        "StaleAppliedFailureIsDisabled": (
            "StaleAppliedTokenState => ~ENABLED "
            "MutationLatchAppliedSuccessorStartupFailure"
        ),
        "InitialStaleRejectionIsEnabled": (
            '(/\\ StaleAppliedTokenState /\\ lastTransition = "Initial") '
            "=> ENABLED FixedRejectStaleSuccessorActivation"
        ),
        "BuggyBeginViolationWitness": (
            'lastTransition = "BuggyBegin" => '
            "~SuccessorActivationProtocolInvariantProjection"
        ),
        "FixedRejectPreservesStaleState": (
            'lastTransition = "FixedReject" => StaleAppliedTokenState'
        ),
        "AppliedFailurePreservesRunningWitness": (
            'lastTransition = "AppliedFailure" => '
            'activationStatus = "Running"'
        ),
        "BugMutationNext": "BuggyBeginSuccessorActivation",
        "BugMutationSpec": (
            "StaleAppliedTokenInit /\\ "
            "[][BugMutationNext]_MutationVars"
        ),
        "FixedMutationNext": (
            "\\/ FixedBeginSuccessorActivation "
            "\\/ FixedRejectStaleSuccessorActivation "
            "\\/ MutationLatchAppliedSuccessorStartupFailure"
        ),
        "FixedMutationSpec": (
            "StaleAppliedTokenInit /\\ "
            "[][FixedMutationNext]_MutationVars"
        ),
    }
    for symbol, exact in exact_operators.items():
        require_operator(symbol, exact=exact)
    cfg_contracts = {
        bug_cfg_path: (
            "SPECIFICATION BugMutationSpec",
            "CHECK_DEADLOCK FALSE",
            "INVARIANT MutationTypeInvariant",
            "INVARIANT StaleBuggyBeginIsEnabled",
            "INVARIANT StaleFixedBeginIsDisabled",
            "INVARIANT StaleAppliedFailureIsDisabled",
            "INVARIANT BuggyBeginViolationWitness",
            "INVARIANT SuccessorActivationProtocolInvariantProjection",
        ),
        fixed_cfg_path: (
            "SPECIFICATION FixedMutationSpec",
            "CHECK_DEADLOCK FALSE",
            "INVARIANT MutationTypeInvariant",
            "INVARIANT StaleFixedBeginIsDisabled",
            "INVARIANT StaleAppliedFailureIsDisabled",
            "INVARIANT InitialStaleRejectionIsEnabled",
            "INVARIANT FixedRejectPreservesStaleState",
            "INVARIANT AppliedFailurePreservesRunningWitness",
            "INVARIANT SuccessorActivationProtocolInvariantProjection",
        ),
    }
    for cfg_path, expected_lines in cfg_contracts.items():
        if not cfg_path.is_file() or cfg_path.is_symlink():
            continue
        actual_lines = tuple(
            line.strip()
            for line in cfg_path.read_text(encoding="utf-8").splitlines()
            if line.strip() and not line.lstrip().startswith("\\*")
        )
        if actual_lines != expected_lines:
            errors.append(
                f"{cfg_path}: successor stale-token mutation configuration "
                f"must equal {expected_lines!r}; found {actual_lines!r}"
            )
    return errors
