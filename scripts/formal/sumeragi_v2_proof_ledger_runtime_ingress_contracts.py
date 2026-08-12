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
    next_context = (("impl", "Iterator", "for", "OuterIngressTurns"),)
    next_items = tuple(
        item
        for item in rust_items(runner_source, "next")
        if item.brace_context == next_context
    )
    if len(next_items) != 1:
        errors.append(
            f"{runner_path}: require exactly one outer-ingress cursor "
            f"Iterator::next implementation; found {len(next_items)}"
        )
        next_item = None
    else:
        next_item = next_items[0]
        _require_rust_item_context(
            runner_path,
            next_item,
            next_context,
            "finite outer-ingress cursor alternation",
            errors,
        )
    _require_exact_rust_tokens(
        runner_path,
        next_item,
        """
fn next(&mut self) -> Option<Self::Item> {
    if self.cycles_remaining == 0 {
        return None;
    }
    let turn = self.next_turn;
    self.next_turn = match turn {
        OuterIngressTurn::Completion => OuterIngressTurn::Runtime,
        OuterIngressTurn::Runtime => OuterIngressTurn::Ingress,
        OuterIngressTurn::Ingress => {
            self.cycles_remaining -= 1;
            OuterIngressTurn::Completion
        }
    };
    Some(turn)
}
""",
        "ordinary ingress must alternate finite Completion, Runtime, and Ingress turns",
        errors,
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


_REMOTE_PROPOSAL_REPLAY_ITEM_SHA256 = {
    "origin_new": "31a61793dfe490059e20eb77f75b9438f95386adc4a618c9eecc695eb9f3f4d7",
    "origin_rebind": "03b2c047f695feded9b1585a5f0bfa4cee5279f11c169b47bf2a9589631a9e9e",
    "origin_merge": "47cbedbb2fcd3da2122459143e65ed6196d14b7779e29d12d135d3fd7e7157f7",
    "origin_bind_fetch": "546e2a076aeccc4577fc4fcd31898a0cb645fcf2b086cf2a9ca825d7085c4a8b",
    "driver_default_bind": "8a0e749888af5c64b474d89b3048fcb1d91344776319e61c67415f4eeb894bcd",
    "driver_bind": "8117b5b9453be5494bc46f3ed9d164a1472411e6ed712067afcc554f32b6a1e3",
    "ownership_fetch_replay": "fec923e08dd4c0c33198a53369d808298c2a62d8a9abb2ef2aff6ac0c54ba182",
    "authority_mint": "ea8680ad00ae88245688355cc371724b4a133b112be43c3d46fe50f9cc4785e3",
    "authority_match": "fffb59ff81f0cdf5b0f8c1c222693c0fb586af021b2c68746ca5e846bf0ba8e5",
    "authority_pending": "cbea73229aa28cb8524d08e36bc0e89dabfe803f16791dfb1ea9519376082b82",
    "authority_fetch": "95992baebcab3fd7ecce16e4530609d27197b10e58b222764d0abc3e3acc3aae",
    "runtime_regression": "46c5d09beb1f80e9d5c7b2c83a8bf2d8dbfefa56705d915a8899f928a177d398",
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
        "Result<(), ()> { Err(()) }",
        "synthetic runtime drivers must not mint remote Proposal replay authority",
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
    require_order(
        runtime_path,
        production_bind,
        (
            "if effects.len() != ownership.len()",
            "AdapterEffect::FetchBody { certificate: None, .. }",
            "let Some((index, effect)) = fetches.next() else",
            "if fetches.next().is_some() || ownership[index].remote_proposal_fetch_replay.is_some()",
            "ownership[index].pending_adapter_effect_binding(effect)",
            "origin.bind_exact_fetch(effect, pending)",
            "ownership[index].remote_proposal_fetch_replay = Some(replay);",
        ),
        "ordinary Proposal replay must bind at most one exact uncertified Fetch",
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
    retain = next(
        (
            item
            for item in rust_items(runtime_source, "retain_effect_ownership")
            if item.brace_context == generic_context
        ),
        None,
    )
    if retain is None:
        errors.append(f"{runtime_path}: require exactly one runtime effect ownership retainer")
    else:
        _require_rust_token_sequence(
            runtime_path,
            retain,
            """
if effects.is_empty() {
    if let Some(origin) = self.pending_remote_proposal_replay.take() {
        self.driver.bind_remote_proposal_fetch_replay(origin, effects, &mut [])
""",
            "effect ownership must consume an empty pending Proposal replay through the closed binder",
            errors,
        )
        require_order(
            runtime_path,
            retain,
            (
                "let mut ownership = Vec::with_capacity(effects.len());",
                """
if let Some(origin) = self.pending_remote_proposal_replay.take() {
    self.driver.bind_remote_proposal_fetch_replay(origin, effects, &mut ownership)
""",
                "self.pending_effect_ownership = Some(ownership);",
            ),
            "effect ownership must consume the pending Proposal replay before batch publication",
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
let pending = self.pending_adapter_effect_binding(effect)?;
replay.exactly_matches_fetch_pending(effect, &pending).then(|| replay.clone())
""",
        "remote Proposal replay accessor must require the exact Fetch pending binding",
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

    return errors
