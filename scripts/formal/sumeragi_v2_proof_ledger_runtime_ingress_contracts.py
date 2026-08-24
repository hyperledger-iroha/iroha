"""Leader-wire and exact-serve runtime source-fidelity contracts."""
def _outer_ingress_cursor_source_fidelity_errors(
    runner_path: Path,
    runner_source: str,
    outer_turns: RustItem | None,
) -> list[str]:
    """Bind the context-frozen finite Completion/Runtime/Ingress cursor."""

    errors: list[str] = []
    _require_exact_rust_tokens(
        runner_path,
        outer_turns,
        """
fn outer_ingress_turns(
    limit: usize,
    context_id: wire::HeightContextId,
    height: wire::Height,
) -> OuterIngressTurns {
    OuterIngressTurns::new(limit, context_id, height)
}
""",
        "ordinary ingress turn construction must delegate to the context-bound finite cursor",
        errors,
    )
    new_context = (("impl", "OuterIngressTurns"),)
    new_items = tuple(
        item
        for item in rust_items(runner_source, "new")
        if item.brace_context == new_context
    )
    if len(new_items) != 1:
        errors.append(
            f"{runner_path}: require exactly one context-bound outer-ingress "
            f"cursor constructor; found {len(new_items)}"
        )
        new_item = None
    else:
        new_item = new_items[0]
        _require_rust_item_context(
            runner_path,
            new_item,
            new_context,
            "context-bound outer-ingress cursor constructor",
            errors,
        )
    _require_exact_rust_tokens(
        runner_path,
        new_item,
        """
fn new(limit: usize, context_id: wire::HeightContextId, height: wire::Height) -> Self {
    Self {
        context_id,
        height,
        cycles_remaining: limit.max(1),
        next_turn: OuterIngressTurn::Completion,
    }
}
""",
        "outer-ingress cursor must freeze its height context and start at Completion with a positive finite cycle bound",
        errors,
    )
    def exact_context_item(
        name: str,
        context: tuple[tuple[str, ...], ...],
        description: str,
        *,
        expected_attributes: tuple[str, ...] = (),
    ) -> RustItem | None:
        matches = tuple(
            item for item in rust_items(runner_source, name) if item.brace_context == context
        )
        if len(matches) != 1:
            errors.append(f"{runner_path}: require exactly one {description}; found {len(matches)}")
            return None
        item = matches[0]
        _require_rust_item_context(
            runner_path,
            item,
            context,
            description,
            errors,
            expected_attributes=expected_attributes,
        )
        return item

    current_turn_structs = rust_struct_items(runner_source, "LifecycleCurrentRunnerTurn")
    if len(current_turn_structs) != 1:
        errors.append(
            f"{runner_path}: require exactly one borrow-bound current runner turn; "
            f"found {len(current_turn_structs)}"
        )
        current_turn_struct = None
    else:
        current_turn_struct = current_turn_structs[0]
        _require_rust_item_context(
            runner_path,
            current_turn_struct,
            (),
            "borrow-bound current runner turn",
            errors,
            expected_attributes=(
                "#[derive(Debug)]",
                '#[must_use = "the current runner turn must be serviced before the cursor advances"]',
            ),
        )
    _require_exact_rust_tokens(
        runner_path,
        current_turn_struct,
        """
pub(crate) struct LifecycleCurrentRunnerTurn<'cursor> {
    cursor: &'cursor mut OuterIngressTurns,
    turn: OuterIngressTurn,
}
""",
        "current runner turn must retain the sole mutable cursor borrow and exact turn",
        errors,
    )
    owner_context = (("impl", "OuterIngressTurns"),)
    current_context = (("impl", "LifecycleCurrentRunnerTurn", "<", "'", "_", ">"),)
    drop_context = (
        ("impl", "Drop", "for", "LifecycleCurrentRunnerTurn", "<", "'", "_", ">"),
    )
    next_item = exact_context_item(
        "next_current", owner_context, "borrow-bound outer-ingress current-turn mint"
    )
    _require_exact_rust_tokens(
        runner_path,
        next_item,
        """
fn next_current(&mut self) -> Option<LifecycleCurrentRunnerTurn<'_>> {
    if self.cycles_remaining == 0 {
        return None;
    }
    Some(LifecycleCurrentRunnerTurn {
        turn: self.next_turn,
        cursor: self,
    })
}
""",
        "current-turn mint must freeze the exact next turn under the sole mutable cursor borrow",
        errors,
    )
    turn_item = exact_context_item(
        "turn",
        current_context,
        "test-only borrow-bound outer-ingress turn projection",
        expected_attributes=("#[cfg(test)]",),
    )
    _require_exact_rust_tokens(
        runner_path,
        turn_item,
        """
const fn turn(&self) -> OuterIngressTurn {
    self.turn
}
""",
        "borrow-bound turn projection must expose only its frozen current turn",
        errors,
    )
    advance_item = exact_context_item(
        "advance_current", owner_context, "borrow-bound outer-ingress cursor advance"
    )
    _require_exact_rust_tokens(
        runner_path,
        advance_item,
        """
fn advance_current(&mut self, turn: OuterIngressTurn) {
    assert_eq!(self.next_turn, turn, "borrow-bound outer runner turn must remain current until drop");
    self.next_turn = match turn {
        OuterIngressTurn::Completion => OuterIngressTurn::Runtime,
        OuterIngressTurn::Runtime => OuterIngressTurn::Ingress,
        OuterIngressTurn::Ingress => {
            self.cycles_remaining -= 1;
            OuterIngressTurn::Completion
        }
    };
}
""",
        "cursor advance must preserve Completion/Runtime/Ingress and decrement only after Ingress",
        errors,
    )
    drop_item = exact_context_item(
        "drop", drop_context, "borrow-bound outer-ingress current-turn Drop"
    )
    _require_exact_rust_tokens(
        runner_path,
        drop_item,
        """
fn drop(&mut self) {
    self.cursor.advance_current(self.turn);
}
""",
        "current-turn Drop must advance exactly the still-current borrowed turn",
        errors,
    )
    legacy_next_context = (("impl", "Iterator", "for", "OuterIngressTurns"),)
    legacy_next = tuple(
        item for item in rust_items(runner_source, "next")
        if item.brace_context == legacy_next_context
    )
    if legacy_next:
        errors.append(
            f"{runner_path}: borrow-bound outer-ingress cursor may not retain an "
            f"independent Iterator::next bypass; found {len(legacy_next)}"
        )
    rank_item = _require_rust_item(
        runner_path, runner_source, "outer_ingress_turn_index", errors
    )
    _require_exact_rust_tokens(
        runner_path,
        rank_item,
        """
const fn outer_ingress_turn_index(turn: OuterIngressTurn) -> u8 {
    match turn {
        OuterIngressTurn::Completion => 0,
        OuterIngressTurn::Runtime => 1,
        OuterIngressTurn::Ingress => 2,
    }
}
""",
        "outer-ingress cursor rank must use the same Completion, Runtime, and Ingress order",
        errors,
    )
    return errors


def _borrow_bound_outer_ingress_order_errors(
    drain_path: Path,
    drain: RustItem | None,
) -> list[str]:
    """Require one current-turn borrow and Runtime service before ingress."""

    sequences = (
        "let mut outer_turns = outer_ingress_turns(limit, context_id, height);",
        "while let Some(current_turn) = outer_turns.next_current()",
        "match current_turn.target()",
        "LifecycleRunnerRankTarget::Completion =>",
        "LifecycleRunnerRankTarget::Runtime =>",
        "advance_executor(receiver, owner, executor, services, 1)?",
        "LifecycleRunnerRankTarget::Ingress =>",
        "activated.drive_ingress_turn(current_turn)",
    )
    tokens = rust_code_tokens("" if drain is None else drain.source)
    positions = tuple(
        _token_sequence_positions(tokens, rust_code_tokens(sequence))
        for sequence in sequences
    )
    if any(len(found) != 1 for found in positions) or any(
        left[0] >= right[0]
        for left, right in zip(positions, positions[1:])
        if left and right
    ):
        return [
            f"{drain_path}: every ordinary outer ingress occurrence must be preceded "
            "by the borrow-bound Completion/Runtime lifecycle turns and serialized "
            "advance_executor turn before the single ingress owner"
        ]
    return []


_REMOTE_PROPOSAL_REPLAY_ITEM_SHA256 = {
    "origin_new": "31a61793dfe490059e20eb77f75b9438f95386adc4a618c9eecc695eb9f3f4d7",
    "origin_rebind": "03b2c047f695feded9b1585a5f0bfa4cee5279f11c169b47bf2a9589631a9e9e",
    "origin_merge": "47cbedbb2fcd3da2122459143e65ed6196d14b7779e29d12d135d3fd7e7157f7",
    "origin_exact_proposal": "fbd6758c3525703a5745b3b40eeb927a96ec367e308e58bb0c8a8c8c6453ed02",
    "origin_same_proposal": "6c4d46c6586299b736f4478e826d73b0f38cce7a14baa660b2c13b661d474e12",
    "origin_bind_fetch": "546e2a076aeccc4577fc4fcd31898a0cb645fcf2b086cf2a9ca825d7085c4a8b",
    "driver_default_bind": "cf110b00fc4ad92d34d38ffb0eea34978c86c26da90eb84a024fb7b3ad12b39a",
    "driver_default_dormant": "0605b8ac38c3bca1ee626c34fa85fa58d119ce0838efffbc9c4cbfb68a4197d6",
    "driver_bind": "70439d768373a918a782110cda58f8c0313c4cbab376aadab1a5d2eb194e086e",
    "driver_dormant": "ac4c86088f8913b976b7419c06bfc49dc5276bf6f4c403e3bd99cf86348808d4",
    "adapter_dormant": "54d8f963420cde8fcfda352272364ab660b0cb23b8e4eedf77f96ff295cc9b90",
    "reducer_dormant": "f4c5c9082a99cdb25570683a7fcf18d672ee408c83a59f4aa21853393e0a4fc8",
    "runtime_reconcile_dormant": "9daa974de6152a5652ba1543555f7b39c483aff9ea6146abe23ce553cab5b648",
    "runtime_retain_dormant": "7cafc6b25ac53ab494768e72b2b562f3110c4318d8fd36eae4f49a15d61cbca9",
    "runtime_bind_or_retain": "2162d953f76cb34cb9ee5246d7731e9543cc980676ad64367ba0989ca260e93e",
    "runtime_retain_effect": "dbf5d0d4fca1ff40aa9b62fed25847bd0f0cca5516b05c3e8e7dca17050c1d96",
    "candidate_retry_adopt": "ecda2cc6f6dc290bdf930ec16f943645f65a40fe8c11033ff29f3e4d181b1f33",
    "fetch_consumer_rebind": "fae6445791cb8e360adab46ae142cc4d3e291ddd40ed55393eeb67d369e61b18",
    "fetch_authority_adopt": "e846fda5841a09586c8baded336d1723f2cc2d625a1078ff21e8185913cb38b1",
    "body_fetch_rebind": "eba1c4dbbc4292774bfb50861995e9fb212643576d8522e318a91efa7cec6399",
    "executor_retain": "2d7cbd5e2c9bcd323c60925770d8a9406ea85877cacac7b6a323729df07e3bde",
    "executor_ready": "8f91405f9c0ac870564dd0334596e55c8c9ccce435ee2a7684c7a22c32cde403",
    "ownership_fetch_replay": "1ac9ff4ff448a03d9348faab7796bc1c986cd0600f64d3feb27b4f0abee657c1",
    "authority_mint": "ea8680ad00ae88245688355cc371724b4a133b112be43c3d46fe50f9cc4785e3",
    "authority_match": "fffb59ff81f0cdf5b0f8c1c222693c0fb586af021b2c68746ca5e846bf0ba8e5",
    "authority_pending": "cbea73229aa28cb8524d08e36bc0e89dabfe803f16791dfb1ea9519376082b82",
    "authority_fetch": "95992baebcab3fd7ecce16e4530609d27197b10e58b222764d0abc3e3acc3aae",
    "runtime_regression": "9ae4b804a0b5bcae727b7ee1a6839099bf5c8b7b2d2704cd6e849b14fa406f69",
    "set_b_regression": "ab363d30b5f8743b4b26b2bb5e52d7db9bbfb61aa43bcac2a8452a16c091a231",
    "protected_rebind_regression": "779d05e40b2afedee904dcf6d7396de85ce1a8d4e33c2e168e2d9b2fb07e3144",
}


def _remote_proposal_replay_source_fidelity_errors(
    repo_root: Path,
    runtime_path: Path,
    runtime_source: str,
) -> list[str]:
    """Bind authenticated remote Proposal ingress to its one ordinary Fetch."""

    errors: list[str] = []
    authority_path = runtime_path.with_name("v2_lifecycle_replay_authority.rs")
    if not authority_path.is_file() or authority_path.is_symlink():
        return [f"{authority_path}: remote Proposal replay authority source is required"]
    _path, authority_source = _read_reviewed_rust_source(
        repo_root,
        authority_path.relative_to(repo_root).as_posix(),
        errors,
        "remote Proposal replay authority source",
    )
    adapter_path = runtime_path.with_name("v2.rs")
    _path, adapter_source = _read_reviewed_rust_source(
        repo_root,
        adapter_path.relative_to(repo_root).as_posix(),
        errors,
        "remote Proposal dormant-candidate adapter source",
    )
    reducer_path = runtime_path.with_name("v2_core") / "reducer.rs"
    _path, reducer_source = _read_reviewed_rust_source(
        repo_root,
        reducer_path.relative_to(repo_root).as_posix(),
        errors,
        "remote Proposal dormant-candidate reducer source",
    )
    effects_path = runtime_path.with_name("v2_effects.rs")
    _path, effects_source = _read_reviewed_rust_source(
        repo_root,
        effects_path.relative_to(repo_root).as_posix(),
        errors,
        "remote Proposal dormant-candidate executor source",
    )

    def one_item(
        path: Path,
        source: str,
        name: str,
        context: tuple[tuple[str, ...], ...],
        digest_key: str,
        description: str,
        *,
        attributes: tuple[str, ...] = (),
    ) -> RustItem | None:
        matches = tuple(
            item for item in rust_items(source, name) if item.brace_context == context
        )
        if len(matches) != 1:
            errors.append(f"{path}: require exactly one {description}; found {len(matches)}")
            return None
        item = matches[0]
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
            _REMOTE_PROPOSAL_REPLAY_ITEM_SHA256[digest_key],
            description,
            errors,
        )
        return item

    def require_order(
        path: Path,
        item: RustItem | None,
        sequences: tuple[str, ...],
        diagnostic: str,
    ) -> None:
        if item is None:
            return
        tokens = rust_code_tokens(item.body)
        positions = [
            _token_sequence_positions(tokens, rust_code_tokens(sequence))
            for sequence in sequences
        ]
        if any(len(found) != 1 for found in positions) or any(
            left[0] >= right[0]
            for left, right in zip(positions, positions[1:])
            if left and right
        ):
            errors.append(f"{path}:{item.line}: {diagnostic}")

    runtime_tokens = rust_code_tokens(runtime_source)
    for declaration, description in (
        (
            "remote_proposal_fetch_replay: Option<RemoteProposalFetchReplayEvidenceV1>",
            "effect ownership must retain exactly one opaque remote Proposal replay sidecar",
        ),
        (
            "remote_proposal_replay: Option<AuthenticatedRemoteProposalDispatchOrigin>",
            "driver dispatch must carry exactly one opaque remote Proposal origin",
        ),
        (
            "deferred_remote_proposal_replay: BTreeMap<u128, AuthenticatedRemoteProposalDispatchOrigin>",
            "serialized runtime must retain one bounded deferred Proposal map",
        ),
        (
            "pending_remote_proposal_replay: Option<AuthenticatedRemoteProposalDispatchOrigin>",
            "serialized runtime must retain one immediate Proposal binding slot",
        ),
        (
            "dormant_remote_proposal_replay: Option<AuthenticatedRemoteProposalDispatchOrigin>",
            "serialized runtime must retain one bounded dormant Proposal binding slot",
        ),
    ):
        count = _token_sequence_count(runtime_tokens, rust_code_tokens(declaration))
        if count != 1:
            errors.append(f"{runtime_path}: {description}; found {count}")

    origin_context = (("impl", "AuthenticatedRemoteProposalDispatchOrigin"),)
    origin_new = one_item(
        runtime_path,
        runtime_source,
        "new",
        origin_context,
        "origin_new",
        "authenticated remote Proposal origin constructor",
    )
    origin_rebind = one_item(
        runtime_path,
        runtime_source,
        "rebind_retained_ingress",
        origin_context,
        "origin_rebind",
        "authenticated remote Proposal retained-ingress rebind",
    )
    origin_merge = one_item(
        runtime_path,
        runtime_source,
        "merge_retained",
        origin_context,
        "origin_merge",
        "authenticated remote Proposal deferred merge",
    )
    origin_exact_proposal = one_item(
        runtime_path,
        runtime_source,
        "exact_proposal",
        origin_context,
        "origin_exact_proposal",
        "authenticated remote Proposal exact-envelope accessor",
    )
    origin_same_proposal = one_item(
        runtime_path,
        runtime_source,
        "same_authenticated_proposal",
        origin_context,
        "origin_same_proposal",
        "authenticated remote Proposal duplicate matcher",
    )
    origin_bind = one_item(
        runtime_path,
        runtime_source,
        "bind_exact_fetch",
        origin_context,
        "origin_bind_fetch",
        "authenticated remote Proposal Fetch mint",
    )
    _require_rust_token_sequence(
        runtime_path,
        origin_new,
        """
if !matches!(authenticated.payload(), wire::ConsensusMessageV2Payload::Proposal(_))
    || !ingress.exactly_matches_authenticated(&authenticated)
{
    return None;
}
Some(Self {
    authenticated,
    ingress,
})
""",
        "remote Proposal origin must bind one authenticated Proposal to its exact frozen ingress",
        errors,
    )
    require_order(
        runtime_path,
        origin_rebind,
        (
            "retained.exactly_matches_authenticated(&self.authenticated)",
            "let mut merged = self.ingress.clone();",
            "merged.merge_downstream(retained.clone()).ok()?;",
            "if merged != retained",
            "ingress: retained",
        ),
        "remote Proposal deferred rebase must preserve the same envelope and exact retained ingress",
    )
    require_order(
        runtime_path,
        origin_merge,
        (
            "self.authenticated.same_wire_envelope(&incumbent.authenticated)",
            "incumbent.rebind_retained_ingress(retained.clone())",
            "self.rebind_retained_ingress(retained)",
        ),
        "remote Proposal deferred merge must preserve one envelope and retained ingress",
    )
    _require_rust_token_sequence(
        runtime_path,
        origin_exact_proposal,
        "self.ingress.exactly_matches_authenticated(&self.authenticated).then_some(proposal)",
        "dormant Proposal access must reauthenticate the frozen ingress envelope",
        errors,
    )
    _require_rust_token_sequence(
        runtime_path,
        origin_same_proposal,
        "self.exact_proposal().is_some() && other.exact_proposal().is_some() && self.authenticated.same_wire_envelope(&other.authenticated)",
        "dormant Proposal coalescence must preserve the complete authenticated envelope",
        errors,
    )
    _require_rust_token_sequence(
        runtime_path,
        origin_bind,
        """
RemoteProposalFetchReplayEvidenceV1::from_exact_authenticated_proposal(
    RemoteProposalReplayMintPermit::new(),
    self.authenticated,
    self.ingress,
    effect,
    pending,
)
""",
        "remote Proposal Fetch mint must consume the private one-shot permit and complete origin",
        errors,
    )

    trait_context = (("pub", "(", "crate", ")", "trait", "RuntimeDriver"),)
    default_bind = one_item(
        runtime_path,
        runtime_source,
        "bind_remote_proposal_fetch_replay",
        trait_context,
        "driver_default_bind",
        "closed synthetic remote Proposal replay binder",
    )
    _require_rust_token_sequence(
        runtime_path,
        default_bind,
        "Result<Option<AuthenticatedRemoteProposalDispatchOrigin>, ()> { Err(()) }",
        "synthetic runtime drivers must not mint remote Proposal replay authority",
        errors,
    )
    default_dormant = one_item(
        runtime_path,
        runtime_source,
        "remote_proposal_fetch_replay_is_dormant",
        trait_context,
        "driver_default_dormant",
        "closed synthetic dormant remote Proposal classifier",
    )
    _require_rust_token_sequence(
        runtime_path,
        default_dormant,
        "false",
        "synthetic runtime drivers must reject dormant Proposal authority by default",
        errors,
    )
    production_context = (("impl", "RuntimeDriver", "for", "SumeragiV2Adapter"),)
    production_bind = one_item(
        runtime_path,
        runtime_source,
        "bind_remote_proposal_fetch_replay",
        production_context,
        "driver_bind",
        "production remote Proposal replay binder",
    )
    production_dormant = one_item(
        runtime_path,
        runtime_source,
        "remote_proposal_fetch_replay_is_dormant",
        production_context,
        "driver_dormant",
        "production dormant remote Proposal classifier",
    )
    require_order(
        runtime_path,
        production_bind,
        (
            "if effects.len() != ownership.len()",
            "AdapterEffect::FetchBody { certificate: None, .. }",
            "let Some((index, effect)) = fetches.next() else",
            "self.remote_proposal_fetch_replay_is_dormant(&origin)",
            "if fetches.next().is_some() || ownership[index].remote_proposal_fetch_replay.is_some()",
            "ownership[index].exact_pending_adapter_effect_binding(effect)",
            "origin.bind_exact_fetch(effect, pending)",
            "ownership[index].remote_proposal_fetch_replay = Some(replay);",
            "Ok(None)",
        ),
        "ordinary Proposal replay must bind at most one exact uncertified Fetch",
    )
    _require_rust_token_sequence(
        runtime_path,
        production_dormant,
        "origin.exact_proposal().is_some_and(|proposal| self.retains_dormant_remote_proposal_fetch(proposal))",
        "production dormant replay must require the exact authenticated Proposal and closed adapter candidate",
        errors,
    )

    adapter_dormant = one_item(
        adapter_path,
        adapter_source,
        "retains_dormant_remote_proposal_fetch",
        (("impl", "SumeragiV2Adapter"),),
        "adapter_dormant",
        "adapter dormant remote Proposal candidate matcher",
    )
    require_order(
        adapter_path,
        adapter_dormant,
        (
            "self.reducer.dormant_set_b_proposal_fetch_candidate()",
            "self.registry.clone()",
            "signed_proposal_to_wire(candidate, self.aggregator.as_ref())",
            "candidate == *proposal",
        ),
        "adapter dormant replay must compare the complete reconstructed signed Proposal",
    )
    reducer_dormant = one_item(
        reducer_path,
        reducer_source,
        "dormant_set_b_proposal_fetch_candidate",
        (("impl", "Reducer"),),
        "reducer_dormant",
        "closed Set-B dormant Proposal candidate classifier",
        attributes=("#[must_use]",),
    )
    for sequence, diagnostic in (
        ("self.durable.decision().is_none()", "a Decision must retire dormant Proposal authority"),
        ("self.pending_persistence.is_none()", "pending persistence must exclude dormant Proposal authority"),
        ("self.durable.timeout_intent(round).is_none()", "timeout intent must exclude dormant Proposal authority"),
        ("round == Round::new(self.context.height(), self.durable.current_view())", "dormant Proposal authority must be current-round only"),
        ("self.local_committee_role() == Some(CommitteeRole::SetBValidator)", "only Set B may retain dormant Proposal authority"),
        ("!self.fallback_active", "activated fallback is no longer dormant"),
        ("self.body_state(round, subject) == BodyState::Missing", "dormant Proposal authority requires missing body work"),
        ("!self.pending_prepare.contains_key(&certificate)", "Prepare authority must retire dormant Proposal authority"),
    ):
        _require_rust_token_sequence(
            reducer_path,
            reducer_dormant,
            sequence,
            diagnostic,
            errors,
        )
    dispatch = next(
        (
            item
            for item in rust_items(runtime_source, "dispatch")
            if item.brace_context == production_context
        ),
        None,
    )
    require_order(
        runtime_path,
        dispatch,
        (
            "if authenticated != tagged.ingress_ownership.is_some()",
            "let remote_proposal_replay = match (&tagged.command, &tagged.ingress_ownership)",
            "wire::ConsensusMessageV2Payload::Proposal(_)",
            "AuthenticatedRemoteProposalDispatchOrigin::new(",
            "message.clone()",
            "ingress.clone()",
            "self.bind_selected_producer_lifecycle(",
            "self.clear_selected_producer_lifecycle();",
            "let outcome = outcome?;",
            "remote_proposal_replay,",
        ),
        "authenticated Proposal dispatch must derive and transfer one exact replay origin",
    )

    generic_context = (
        ("impl", "<", "D", ":", "RuntimeDriver", ">", "SerializedV2Runtime", "<", "D", ">"),
    )
    generic_items: dict[str, RustItem | None] = {}
    for name in (
        "reconcile_deferred_runtime_ownership_after_retirement",
        "accept_driver_dispatch",
        "dispatch_one_adapter_deferred",
    ):
        matches = tuple(
            item
            for item in rust_items(runtime_source, name)
            if item.brace_context == generic_context
        )
        if len(matches) != 1:
            errors.append(f"{runtime_path}: require exactly one remote Proposal replay {name}; found {len(matches)}")
            generic_items[name] = None
        else:
            generic_items[name] = matches[0]
    require_order(
        runtime_path,
        generic_items["reconcile_deferred_runtime_ownership_after_retirement"],
        (
            "let active = self.driver.all_deferred_admission_ordinals();",
            "let authenticated = self.driver.authenticated_deferred_admission_ordinals();",
            "self.deferred_remote_proposal_replay.retain(|ordinal, _| authenticated.contains(ordinal));",
            "self.deferred_remote_proposal_replay.iter().any(|(ordinal, origin)|",
            "ingress.get(ordinal) != Some(&origin.ingress)",
            "origin.ingress.exactly_matches_authenticated(&origin.authenticated)",
            "self.deferred_lifecycle_ownership = lifecycle;",
            "self.deferred_ingress_ownership = ingress;",
        ),
        "retirement reconciliation must prune and validate deferred Proposal replay",
    )
    require_order(
        runtime_path,
        generic_items["accept_driver_dispatch"],
        (
            "if self.pending_remote_proposal_replay.is_some()",
            "self.reconcile_deferred_ingress_ownership(deferred_ingress)",
            "self.deferred_remote_proposal_replay.retain(|ordinal, _| active.contains(ordinal));",
            "match (retry_unadmitted, deferred_ordinal, remote_proposal_replay)",
            "origin.merge_retained(incumbent, retained_ingress)",
            "origin.rebind_retained_ingress(retained_ingress)",
            "self.deferred_remote_proposal_replay.insert(ordinal, origin);",
            "self.pending_remote_proposal_replay = Some(origin);",
            "self.deferred_remote_proposal_replay.iter().any(|(ordinal, origin)|",
            "origin.ingress.exactly_matches_authenticated(&origin.authenticated)",
            "self.deferred_lifecycle_ownership = retained;",
        ),
        "driver acceptance must retain Proposal replay with its exact deferred ingress owner",
    )
    require_order(
        runtime_path,
        generic_items["dispatch_one_adapter_deferred"],
        (
            "self.deferred_ingress_ownership.remove(&evidence.admission_ordinal)",
            "self.deferred_remote_proposal_replay.remove(&evidence.admission_ordinal)",
            "(DeferredEventKind::ProposalReceived, Some(origin), Some(ingress))",
            "origin.rebind_retained_ingress(ingress.clone())",
            "self.retain_scheduler_ownership(",
            "if self.pending_remote_proposal_replay.is_some()",
            "self.pending_remote_proposal_replay = remote_proposal_replay;",
            "self.retain_effect_ownership(",
        ),
        "deferred Proposal replay must rebind the selected ProposalReceived ingress before effect ownership",
    )
    reconcile_dormant = one_item(
        runtime_path,
        runtime_source,
        "reconcile_dormant_remote_proposal_replay",
        generic_context,
        "runtime_reconcile_dormant",
        "serialized dormant Proposal reconciliation",
    )
    retain_dormant = one_item(
        runtime_path,
        runtime_source,
        "retain_dormant_remote_proposal_replay",
        generic_context,
        "runtime_retain_dormant",
        "serialized dormant Proposal retention",
    )
    bind_or_retain = one_item(
        runtime_path,
        runtime_source,
        "bind_or_retain_remote_proposal_replay",
        generic_context,
        "runtime_bind_or_retain",
        "serialized Proposal bind-or-retain handoff",
    )
    retain = one_item(
        runtime_path,
        runtime_source,
        "retain_effect_ownership",
        generic_context,
        "runtime_retain_effect",
        "serialized runtime effect ownership retainer",
    )
    _require_rust_token_sequence(
        runtime_path,
        reconcile_dormant,
        "self.driver.remote_proposal_fetch_replay_is_dormant(origin)",
        "dormant Proposal reconciliation must retain only the exact still-latent candidate",
        errors,
    )
    require_order(
        runtime_path,
        retain_dormant,
        (
            "self.driver.remote_proposal_fetch_replay_is_dormant(&origin)",
            "self.dormant_remote_proposal_replay.take()",
            "self.driver.remote_proposal_fetch_replay_is_dormant(&incumbent)",
            "incumbent.same_authenticated_proposal(&origin)",
            "self.dormant_remote_proposal_replay = Some(retained);",
        ),
        "the one dormant slot must preserve an exact authenticated duplicate and reject replacement",
    )
    require_order(
        runtime_path,
        bind_or_retain,
        (
            "self.driver.bind_remote_proposal_fetch_replay(origin, effects, ownership)",
            "if let Some(dormant) = dormant",
            "self.retain_dormant_remote_proposal_replay(dormant)?;",
        ),
        "Proposal binding must either consume the origin or retain its closed dormant successor",
    )
    require_order(
        runtime_path,
        retain,
        (
            "let dormant_retransmit = if source == RuntimeEffectSource::Retransmit",
            "self.dormant_remote_proposal_replay.take()",
            "self.reconcile_dormant_remote_proposal_replay();",
            "if self.pending_remote_proposal_replay.is_some() && dormant_retransmit.is_some()",
            "if effects.is_empty()",
            ".or(dormant_retransmit)",
            "let mut ownership = Vec::with_capacity(effects.len());",
            """
if let Some(origin) = self.pending_remote_proposal_replay.take() {
    self.bind_or_retain_remote_proposal_replay(origin, effects, &mut ownership)?;
}
""",
            """
if let Some(origin) = dormant_retransmit {
    self.bind_or_retain_remote_proposal_replay(origin, effects, &mut ownership)?;
}
""",
            "self.pending_effect_ownership = Some(ownership);",
        ),
        "only Retransmit may consume dormant Proposal authority, after exact direct-origin exclusion and before batch publication",
    )

    ownership_context = (("impl", "RuntimeEffectOwnership"),)
    accessor = one_item(
        runtime_path,
        runtime_source,
        "exact_remote_proposal_fetch_replay",
        ownership_context,
        "ownership_fetch_replay",
        "exact remote Proposal Fetch replay accessor",
    )
    _require_rust_token_sequence(
        runtime_path,
        accessor,
        """
let replay = self.remote_proposal_fetch_replay.as_ref()?;
let pending = self.exact_pending_adapter_effect_binding(effect).ok()?;
replay.exactly_matches_fetch_pending(effect, &pending).then(|| replay.clone())
""",
        "remote Proposal replay accessor must require the exact Fetch pending binding",
        errors,
    )
    fetch_adopt = one_item(
        runtime_path,
        runtime_source,
        "adopt_incumbent_fetch_for_retry_or_authority",
        ownership_context,
        "fetch_authority_adopt",
        "incumbent Fetch authority adoption",
    )
    require_order(
        runtime_path,
        fetch_adopt,
        (
            "incumbent_statement.fetch_authority_relation_to(incoming_statement)",
            "RuntimeEffectOwnership::new_bound(",
            "relation == RuntimeFetchAuthorityRelation::Same",
            "self.exact_remote_proposal_fetch_replay(effect)",
            "replay.exactly_matches_fetch_pending(effect, &pending)",
            "adopted.remote_proposal_fetch_replay = Some(replay);",
        ),
        "same-authority Fetch adoption must preserve the exact authenticated Proposal replay while upgrades remain replay-neutral",
    )
    candidate_retry_adopt = one_item(
        runtime_path,
        runtime_source,
        "adopt_incumbent_candidate_for_semantic_retry",
        ownership_context,
        "candidate_retry_adopt",
        "incumbent candidate retry adoption",
    )
    require_order(
        runtime_path,
        candidate_retry_adopt,
        (
            "RuntimeEffectOwnership::new_bound(",
            "self.exact_remote_proposal_fetch_replay(effect)",
            "replay.exactly_matches_fetch_pending(effect, &pending)",
            "adopted.remote_proposal_fetch_replay = Some(replay);",
        ),
        "exact candidate retry adoption must preserve an authenticated ordinary Fetch replay owner",
    )
    fetch_consumer_rebind = one_item(
        runtime_path,
        runtime_source,
        "rebind_fetch_consumer",
        ownership_context,
        "fetch_consumer_rebind",
        "authenticated ordinary Fetch consumer rebind",
    )
    require_order(
        runtime_path,
        fetch_consumer_rebind,
        (
            "previous_pending.project_fetch_consumer_rebind(previous_effect, rebound_effect)",
            "RuntimeEffectOwnership::new_bound(",
            "replay.rebind_exact_consumer(",
            "rebound.remote_proposal_fetch_replay = Some(replay);",
        ),
        "ordinary Fetch consumer rebind must project both pending ownership and authenticated replay",
    )

    body_fetch_rebind = one_item(
        effects_path,
        effects_source,
        "rebind_consumer",
        (("impl", "BodyFetchTask"),),
        "body_fetch_rebind",
        "executor body-Fetch consumer rebind",
    )
    require_order(
        effects_path,
        body_fetch_rebind,
        (
            "let previous_effect = self.adapter_effect();",
            "exact_remote_proposal_fetch_replay(&previous_effect)",
            "rebind_fetch_consumer(&previous_effect, &rebound_effect)",
            "rebind_same_adapter_effect(&rebound_effect)",
        ),
        "executor body-Fetch rebind must use the Proposal-only proof exactly when replay authority is present",
    )

    executor_ready = one_item(
        effects_path,
        effects_source,
        "ready_to_finish",
        (("impl", "V2EffectExecutor", "<", "SerializedV2Runtime", ">"),),
        "executor_ready",
        "executor terminal readiness fence",
    )
    _require_rust_token_sequence(
        effects_path,
        executor_ready,
        """
&& !self.runtime.has_dormant_remote_proposal_replay()
&& self.remote_proposal_replay.is_empty()
&& self.authenticated_genesis_replay.is_empty()
""",
        "height finalization must not discard one live dormant Proposal or authenticated-genesis replay origin",
        errors,
    )
    executor_retain = one_item(
        effects_path,
        effects_source,
        "retain_effect_batch_at_frontier",
        (("impl", "<", "R", ":", "EffectRuntime", ">", "V2EffectExecutor", "<", "R", ">"),),
        "executor_retain",
        "executor retained-effect frontier",
    )
    require_order(
        effects_path,
        executor_retain,
        (
            "let effective_tag = entering_view",
            "let effective_ownership = if effective_tag == pending.task.tag",
            "pending.task.rebind_consumer(effective_tag)",
            "effective_ownership.candidate_semantic_identity()",
            "retained_candidate_owners.get_mut(&candidate_identity)",
            "if *candidate_owner != effective_ownership",
            "*candidate_owner = effective_ownership.clone();",
            "retained_fetch_lineages.insert(key, effective_ownership.clone())",
            "incumbent.adopt_incumbent_fetch_for_retry_or_authority(evidence, effect)",
        ),
        "post-EnterView lineage indexing must simulate the exact protected Fetch ownership rebind before Same/Upgrade adoption",
    )
    _require_rust_token_sequence(
        effects_path,
        executor_retain,
        "self.stored_replay_incumbent_validate_ownership(key, effect)?",
        "retained Validate authority must project both ordinary Proposal and authenticated-genesis replay incumbents",
        errors,
    )

    authority_context = (("impl", "RemoteProposalFetchReplayEvidenceV1"),)
    authority_mint = one_item(
        authority_path,
        authority_source,
        "from_exact_authenticated_proposal",
        authority_context,
        "authority_mint",
        "remote Proposal Fetch replay authority mint",
    )
    authority_match = one_item(
        authority_path,
        authority_source,
        "exactly_matches_fetch",
        authority_context,
        "authority_match",
        "remote Proposal Fetch replay matcher",
    )
    authority_pending = one_item(
        authority_path,
        authority_source,
        "exactly_matches_fetch_pending",
        authority_context,
        "authority_pending",
        "remote Proposal Fetch pending-binding matcher",
    )
    authority_fetch = one_item(
        authority_path,
        authority_source,
        "exact_remote_proposal_fetch",
        (),
        "authority_fetch",
        "exact ordinary remote Proposal Fetch classifier",
    )
    require_order(
        authority_path,
        authority_mint,
        (
            "let proposal = exact_remote_proposal_fetch(&authenticated, &ingress, effect)?;",
            "if !pending.exactly_binds_adapter_effect(effect)",
            "canonical_replay_authority(",
            "fetch_pending: Arc::new(pending)",
            "evidence.exactly_matches_fetch(effect).then_some(evidence)",
        ),
        "remote Proposal Fetch replay mint must validate the envelope, pending owner, and canonical authority",
    )
    _require_rust_token_sequence(
        authority_path,
        authority_pending,
        "self.exactly_matches_fetch(effect) && self.fetch_pending.as_ref() == pending",
        "remote Proposal Fetch pending matcher must retain exact effect and pending ownership",
        errors,
    )
    _require_rust_token_sequence(
        authority_path,
        authority_fetch,
        """
ingress.exactly_matches_authenticated(authenticated)
    && certified_sources.is_empty()
    && *round == proposal.round
    && *subject == proposal.subject
    && manifest == &proposal.manifest
    && tag.height() == round.height
    && tag.view() >= round.view
""",
        "remote Proposal Fetch replay must reject certified, foreign-coordinate, or foreign-manifest work",
        errors,
    )
    _require_rust_token_sequence(
        authority_path,
        authority_match,
        "self.fetch_pending.exactly_binds_adapter_effect(effect)",
        "remote Proposal Fetch matcher must retain the exact pending effect owner",
        errors,
    )

    test_context = (("#", "[", "cfg", "(", "test", ")", "]", "mod", "tests"),)
    regression = one_item(
        runtime_path,
        runtime_source,
        "authenticated_remote_proposal_retains_exact_fetch_store_validate_replay_origin",
        test_context,
        "runtime_regression",
        "authenticated remote Proposal Fetch/Store/Validate replay regression",
        attributes=("#[test]", "#[allow(clippy::too_many_lines)]"),
    )
    for sequence, diagnostic in (
        (
            "runtime.enqueue_network(wrong_signature).is_err()",
            "remote Proposal replay regression must reject a substituted signature",
        ),
        (
            "fetch_ownership.exact_remote_proposal_fetch_replay(&foreign_manifest_fetch).is_none()",
            "remote Proposal replay regression must reject a foreign manifest",
        ),
        (
            "fetch_ownership.exact_remote_proposal_fetch_replay(&certified_fetch).is_none()",
            "remote Proposal replay regression must reject a certified Fetch",
        ),
        (
            "fetch_replay.exactly_projects_store(store_effect, &store_pending)",
            "remote Proposal replay regression must project exact Fetch into Store",
        ),
    ):
        _require_rust_token_sequence(runtime_path, regression, sequence, diagnostic, errors)

    set_b_regression = one_item(
        runtime_path,
        runtime_source,
        "set_b_proposal_replay_waits_for_and_authenticates_periodic_fallback_fetch",
        test_context,
        "set_b_regression",
        "Set-B dormant Proposal replay regression",
        attributes=("#[test]",),
    )
    for sequence, diagnostic in (
        (
            "committee.role(*index) == Ok(crate::sumeragi::v2_core::CommitteeRole::SetBValidator)",
            "Set-B replay regression must derive rather than assume the local committee role",
        ),
        (
            "initial_effects.is_empty()",
            "Set-B replay regression must exercise the original Applied([]) transition",
        ),
        (
            "assert!(runtime.has_dormant_remote_proposal_replay());",
            "Set-B replay regression must observe the bounded dormant origin",
        ),
        (
            "RuntimeSelectedOwnerKind::PeriodicTimer",
            "Set-B replay regression must consume the origin only through periodic fallback",
        ),
        (
            "ownership[fetch_position].exact_remote_proposal_fetch_replay(&effects[fetch_position]).is_some()",
            "Set-B periodic Fetch must carry the exact authenticated Proposal owner",
        ),
        (
            "!runtime.has_dormant_remote_proposal_replay()",
            "Set-B periodic Fetch must consume the one dormant slot",
        ),
    ):
        _require_rust_token_sequence(
            runtime_path,
            set_b_regression,
            sequence,
            diagnostic,
            errors,
        )

    protected_rebind_regression = one_item(
        effects_path,
        effects_source,
        "enter_view_and_ordinary_fetch_retry_preserve_authenticated_replay_owner",
        test_context,
        "protected_rebind_regression",
        "post-EnterView ordinary Fetch replay regression",
        attributes=("#[test]",),
    )
    for sequence, diagnostic in (
        (
            "authenticated_proposal_fetch_ownership(&fixture, &ordinary, 9_024)",
            "post-EnterView regression must begin with genuine authenticated Proposal authority",
        ),
        (
            "AdapterEffect::EnterView",
            "post-EnterView regression must execute the protected prefix",
        ),
        (
            "pending.task.ownership().exact_remote_proposal_fetch_replay(&retry).is_some()",
            "post-EnterView Same retry must retain the authenticated Proposal envelope",
        ),
    ):
        _require_rust_token_sequence(
            effects_path,
            protected_rebind_regression,
            sequence,
            diagnostic,
            errors,
        )

    return errors
