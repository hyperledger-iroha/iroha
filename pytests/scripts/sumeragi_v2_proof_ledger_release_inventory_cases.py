# Executed lexically in sumeragi_v2_proof_ledger_test.py; do not collect directly.

def _release_corridor_production_inventory(
    source: str, expected_count: int,
) -> tuple[str, ...]:
    """Read every literal test entry without filtering out an unfamiliar owner."""
    marker = "required_production_liveness_tests=(\n"
    assert source.count(marker) == 1, "production inventory declaration must be unique"
    tail = "\n" + source.split(marker, 1)[1]
    closing = re.search(r"(?m)^[ \t]*\)[^\n]*$", tail)
    assert closing is not None, "production inventory must terminate"
    assert closing.group() == ")", "production inventory closing line must be canonical"
    inventory = tuple(
        line.strip() for line in tail[:closing.start()].splitlines()
        if line.strip()
    )
    assert all(
        re.fullmatch(r"[A-Za-z_][A-Za-z_0-9]*(?:::[A-Za-z_][A-Za-z_0-9]*)+", name)
        for name in inventory
    ), "production inventory entries must be literal qualified Rust test names"
    assert len(inventory) == expected_count, "production inventory count must match its seal"
    assert len(set(inventory)) == len(inventory), "production inventory tests must be unique"
    return inventory

def test_release_corridor_inventory_parser_preserves_all_owners() -> None:
    """The complete source inventory passes before changed-input rejection cases."""
    module = load_checker()
    source = (ROOT_DIR / "scripts/run_sumeragi_v2_release_gates.sh").read_text(encoding="utf-8")
    count = module._PRODUCTION_LIVENESS_RELEASE_COUNT
    inventory = _release_corridor_production_inventory(source, count)
    expected = tuple(source.split("required_production_liveness_tests=(\n", 1)[1].split("\n)", 1)[0].split())
    assert inventory == expected
    native_tests = tuple(name for name in inventory if name.startswith("native_amx::"))
    assert len(native_tests) == dict(
        (owner, size) for _, owner, size in module._PRODUCTION_LIVENESS_RELEASE_MODULE_CONTRACTS
    )["native_amx::participant_application_role_tests"]
    for name in native_tests:
        changed = source.replace(f"  {name}\n", "", 1)
        assert changed != source
        with pytest.raises(AssertionError, match="^production inventory count must match its seal$"):
            _release_corridor_production_inventory(changed, count)
    mutations = (
        (source.replace(f"  {native_tests[0]}\n", f"  {native_tests[1]}\n", 1), "production inventory tests must be unique"),
        (source.replace(native_tests[0], "$(unreviewed_test)", 1), "production inventory entries must be literal qualified Rust test names"),
        (source.replace("required_production_liveness_tests=(\n", "removed_inventory=(\n", 1), "production inventory declaration must be unique"),
        (source + "\nrequired_production_liveness_tests=(\n)\n", "production inventory declaration must be unique"),
        (source.replace(f"  {inventory[-1]}\n)", f"  {inventory[-1]}\n)suffix", 1), "production inventory closing line must be canonical"),
        ("required_production_liveness_tests=(\n", "production inventory must terminate"),
        ("required_production_liveness_tests=(\n)", "production inventory count must match its seal"),
    )
    for changed, diagnostic in mutations:
        assert changed != source
        with pytest.raises(AssertionError, match=f"^{re.escape(diagnostic)}$"):
            _release_corridor_production_inventory(changed, count)
    # Owner authorization belongs to the existing exact module/digest checks.
    # An unfamiliar owner must remain visible to them instead of disappearing.
    unfamiliar = "future_owner::tests::must_remain_visible"
    changed = source.replace(native_tests[0], unfamiliar, 1)
    assert changed != source
    actual = _release_corridor_production_inventory(changed, count)
    assert unfamiliar in actual
    assert native_tests[0] not in actual
    assert len(actual) == len(inventory)



def _replace_late_lane_recovery_tokens(module, source: str, before: str, after: str) -> str:
    """Replace one real code span without depending on Rust line wrapping."""
    masked = module.mask_rust_comments_and_literals(source)
    matches = tuple(module._RUST_TOKEN_RE.finditer(masked))
    tokens = tuple(match.group() for match in matches)
    required = module.rust_code_tokens(before)
    replacement = module.rust_code_tokens(after)
    assert required, "late-lane mutation must select real code"
    assert required != replacement, "late-lane mutation must change code tokens"
    starts = [
        start for start in range(len(tokens) - len(required) + 1)
        if tokens[start:start + len(required)] == required
    ]
    assert len(starts) == 1, "late-lane mutation must select exactly one real code span"
    first = starts[0]
    start, end = matches[first].start(), matches[first + len(required) - 1].end()
    changed = source[:start] + after + source[end:]
    assert changed != source
    assert module.rust_code_tokens(changed) == (
        tokens[:first] + replacement + tokens[first + len(required):]
    ), "late-lane mutation must preserve surrounding code tokens"
    return changed


def _late_lane_recovery_runtime_mutations():
    """Preserve the ten recovery mutations and cover fail-closed read boundaries."""
    capacity = "late canonical lane recovery must set actual capacity one before canonical ownership arrives"
    reconstruction = "late canonical lane recovery must distinguish global body application from lane-certificate durability while preserving reconstruction"
    retained = "late canonical lane recovery must retain incomplete certificate progress in the active predecessor and block successor authority"
    discovery = "late canonical lane recovery must keep one bounded exact certificate-discovery source live across a dropped round while the predecessor stays active"
    durable = "late canonical lane recovery must release successor activation only after the exact certificate and application receipt are durable"
    body_available = 'adapter.proposal_body_available(&proposal).expect("read exact body availability")'
    first_persist = 'assert_eq!(adapter.persist_anchored_sessions().expect("rehydrate the late-applied canonical ownership"), 0, "no certificate exists yet to persist");'
    prepare = 'retained_prepare_qc = lane_qc_for_phase(&proposal, &keys[..3], CertPhase::Prepare);'
    incomplete = 'adapter.durable_lane_rollover_authority(&finality_artifact).expect("inspect incomplete decided-lane authority").is_none()'
    first_discovery = '''
        let _ = adapter.drain_effects(usize::MAX);
        adapter.schedule_retransmission().expect("schedule the first exact missing-certificate discovery round");
        let first_round = adapter.drain_effects(usize::MAX);
    '''
    first_proposal = '''
        first_round.iter().any(|effect| {
            matches!(effect, V2LaneWorkEffect::PostLaneBlock {
                message: BlockMessage::LaneBlockProposal(pending), ..
            } if pending == &proposal)
        })
    '''
    completed_persist = 'assert_eq!(adapter.persist_anchored_sessions().expect("persist recovered certificate and application receipt"), 1);'
    receipt = 'adapter.kura.lane_block_application_receipt_available(&proposal)'
    completed = 'adapter.durable_lane_rollover_authority(&finality_artifact).expect("build recovered decided-lane rollover authority").is_some()'
    repeated_proposal = '''
        adapter.drain_effects(usize::MAX).iter().any(|effect| {
            matches!(effect, V2LaneWorkEffect::PostLaneBlock {
                message: BlockMessage::LaneBlockProposal(pending), ..
            } if pending == &proposal)
        })
    '''
    return (
        (body_available, '!' + body_available, reconstruction),
        (first_persist, first_persist.replace(', 0,', ', 1,'), retained),
        (prepare, prepare.replace('CertPhase::Prepare', 'CertPhase::Commit'), retained),
        (incomplete, incomplete.replace('.is_none()', '.is_some()'), retained),
        (first_discovery, first_discovery.replace('adapter.schedule_retransmission().expect("schedule the first exact missing-certificate discovery round");', 'let _ = &adapter;'), discovery),
        (first_proposal, first_proposal.replace('BlockMessage::LaneBlockProposal', 'BlockMessage::LaneBlockVote'), discovery),
        ('V2LaneIngressOutcome::Inserted', 'V2LaneIngressOutcome::Rejected', durable),
        (completed_persist, completed_persist.replace(', 1);', ', 0);'), durable),
        (receipt, receipt + ' && false', durable),
        (completed, completed.replace('.is_some()', '.is_none()'), durable),
        ('adapter.lane_sessions = LaneBlockSessionCache::new(1);', 'adapter.lane_sessions = LaneBlockSessionCache::new(2);', capacity),
        (body_available, body_available.replace('.expect("read exact body availability")', '.unwrap_or(true)'), reconstruction),
        (incomplete, incomplete.replace('.expect("inspect incomplete decided-lane authority")', '.unwrap_or(None)'), retained),
        (repeated_proposal, repeated_proposal.replace('pending == &proposal', 'pending != &proposal'), discovery),
    )


def test_late_lane_recovery_contract_mutations_authenticate_actual_owner() -> None:
    """Each real-owner mutation must break its exact contract from a passing baseline."""
    module = load_checker()
    path = ROOT_DIR / 'crates/iroha_core/src/sumeragi/v2_lane_work.rs'
    source = path.read_text(encoding='utf-8')
    name = 'globally_applied_lane_body_without_certificate_remains_recoverable'
    items = module.rust_items(source, name)
    assert len(items) == 1
    item = items[0]

    def contract_errors(candidate):
        errors = []
        module._require_late_lane_recovery_runtime_source_contracts(path, candidate, errors)
        return errors

    assert contract_errors(item) == []
    # Rust formatting and comments are not evidence of behavior drift.
    wrapped = item.source.replace('.proposal_body_available(&proposal)', '. /* exact read */ proposal_body_available ( &proposal )', 1)
    assert wrapped != item.source
    wrapped_items = module.rust_items(wrapped, name)
    assert len(wrapped_items) == 1
    assert contract_errors(wrapped_items[0]) == []
    for baseline in (item.source, wrapped):
        for before, after, diagnostic in _late_lane_recovery_runtime_mutations():
            changed = _replace_late_lane_recovery_tokens(module, baseline, before, after)
            changed_items = module.rust_items(changed, name)
            assert len(changed_items) == 1
            actual = changed_items[0]
            assert contract_errors(actual) == [
                f'{path}:{actual.line}: {diagnostic} must occur exactly 1 '
                f'time(s) in the real {name} item; found 0'
            ]

    # Lookalikes inside comments/literals cannot supply an executable target.
    original = 'fn example() { /* value.read() */ let note = r#"value.read()"#; value\n.read(); }'
    changed = _replace_late_lane_recovery_tokens(module, original, 'value.read()', 'value.checked_read()')
    assert changed == original.replace('value\n.read()', 'value.checked_read()')
    for candidate, before, after, diagnostic in (
        (original, '/* only a comment */', 'value.read()', 'late-lane mutation must select real code'),
        (original, 'value.read()', 'value . read()', 'late-lane mutation must change code tokens'),
        (original, 'absent.read()', 'value.read()', 'late-lane mutation must select exactly one real code span'),
        (original.replace('value\n.read();', 'value.read(); value.read();'), 'value.read()', 'value.checked_read()', 'late-lane mutation must select exactly one real code span'),
    ):
        with pytest.raises(AssertionError, match=f'^{re.escape(diagnostic)}$'):
            _replace_late_lane_recovery_tokens(module, candidate, before, after)


def complete_ledger(module):
    ledger = copy.deepcopy(module.load_ledger())
    ledger["machine_checked_completion"] = True
    for obligation in ledger["obligations"]:
        expected_status = (
            module.MACHINE_CHECKED_COMPLETION_EXPECTED_STATUS_BY_ID.get(
                obligation["id"]
            )
        )
        if expected_status is not None:
            obligation["status"] = expected_status
    return ledger


def write_tlaps_fixture_logs(
    module, formal_dir: Path, root_dir: Path, log_dir: Path
):
    """Write canonical positive module and exact-target logs for unit fixtures."""

    log_dir.mkdir(parents=True, exist_ok=True)
    (log_dir / "targets").mkdir(parents=True, exist_ok=True)
    source_manifest_sha256 = module._formal_source_manifest(
        formal_dir, root_dir
    )["sha256"]
    ledger_sha256 = module._proof_ledger_sha256(formal_dir)
    for name in module.RELEASE_PROOF_MODULES:
        (log_dir / f"{name}.preflight.log").write_text(
            "frontend summary passed\n"
            f"{module._tlapm_preflight_marker(name, source_manifest_sha256, ledger_sha256)}\n",
            encoding="utf-8",
        )
        (log_dir / f"{name}.log").write_text(
            "[INFO]: All 1 obligation proved.\n"
            f"{module._tlapm_runner_marker(name, source_manifest_sha256, ledger_sha256)}\n",
            encoding="utf-8",
        )
    for target in module._promotion_target_entries(formal_dir, root_dir):
        (log_dir / "targets" / f"{target['obligation_id']}.log").write_text(
            "[INFO]: All 1 obligation proved.\n"
            + module._tlapm_target_marker(
                target,
                obligations_proved=1,
                source_manifest_sha256=source_manifest_sha256,
                ledger_sha256=ledger_sha256,
            )
            + "\n",
            encoding="utf-8",
        )
    return source_manifest_sha256, ledger_sha256


def build_test_evidence(module, tmp_path: Path):
    formal_dir = tmp_path / "docs" / "formal" / "sumeragi_v2"
    shutil.copytree(module.FORMAL_DIR, formal_dir)
    (formal_dir / "proof_coverage.json").write_text(
        json.dumps(complete_ledger(module), indent=2) + "\n",
        encoding="utf-8",
    )
    log_dir = tmp_path / module.FORMAL_EVIDENCE_LOGICAL_ROOT / "tlaps"
    write_tlaps_fixture_logs(module, formal_dir, tmp_path, log_dir)
    evidence = module.build_release_evidence(
        tlapm_version=module.TLAPM_COMMIT[:7],
        log_dir=log_dir,
        formal_dir=formal_dir,
        root_dir=tmp_path,
    )
    return formal_dir, log_dir, evidence


def complete_cross_tool_ledger(module):
    """Return a synthetic complete ledger using the reviewed cross-tool status."""

    return complete_ledger(module)


def build_cross_tool_fixture(module, tmp_path: Path):
    """Build canonical synthetic component logs for checker-only negative tests."""

    # Materialize compact exact non-vacuous synthetic contracts so the
    # promotion validator and every mutation below run through the full
    # signature/kernel/call-site path without duplicating production sources.
    hardened_contracts = []
    shared_kernel_source = "crates/iroha_core/src/sumeragi/v2_core/refinement.rs"
    for contract in module.CROSS_TOOL_REFINEMENT_CONTRACTS:
        claims = []
        for claim in contract.claims:
            if claim.proof_mode == "total_checked_gate":
                claims.append(claim)
                continue
            kernel = f"synthetic_{claim.verus_theorem}_kernel"
            projection_builder = f"synthetic_{claim.verus_theorem}_projection"
            projection_builder_source = (
                f"pub closed spec fn {projection_builder}(projection: u64) "
                "-> u64 { projection }"
            )
            projection_builder_sha256 = hashlib.sha256(
                "\0".join(
                    module.rust_code_tokens(projection_builder_source)
                ).encode("utf-8")
            ).hexdigest()
            call_source = claim.production_sources[0]
            call_item = f"enforce_{claim.verus_theorem}"
            call_expression = f"assert!({kernel}(projection));"
            synthetic_call_source = (
                f"fn {call_item}(projection: u64) {{\n"
                f"    {call_expression}\n"
                "}\n"
            )
            extracted_call_items = module.rust_items(
                synthetic_call_source, call_item
            )
            assert len(extracted_call_items) == 1
            call_item_sha256 = module._rust_sealed_item_token_sha256(
                extracted_call_items[0]
            )
            claims.append(
                module.CrossToolClaimContract(
                    constant=claim.constant,
                    verus_theorem=claim.verus_theorem,
                    verus_source=claim.verus_source,
                    production_sources=claim.production_sources,
                    verus_parameters="projection: u64",
                    verus_requires="projection > 0",
                    verus_ensures=(
                        f"{kernel}({projection_builder}(projection)), "
                        f"{projection_builder}(projection) >= 1"
                    ),
                    verified_kernel=kernel,
                    verified_kernel_source=shared_kernel_source,
                    verified_kernel_parameters="projection: u64",
                    verified_kernel_body="projection > 0",
                    theorem_kernel_projection=(
                        f"{projection_builder}(projection)"
                    ),
                    theorem_projection_builder=projection_builder,
                    theorem_projection_builder_parameters="projection: u64",
                    theorem_projection_builder_return="u64",
                    theorem_projection_builder_item_sha256=(
                        projection_builder_sha256
                    ),
                    production_call_sites=(
                        module.CrossToolProductionCallContract(
                            source=call_source,
                            item=call_item,
                            projection="projection",
                            required_expression=call_expression,
                            item_token_sha256=call_item_sha256,
                        ),
                    ),
                )
            )
        hardened_contracts.append(
            module.CrossToolObligationContract(
                obligation_id=contract.obligation_id,
                module=contract.module,
                ledger_symbol=contract.ledger_symbol,
                tla_theorem=contract.tla_theorem,
                tla_statement=contract.tla_statement,
                claims=tuple(claims),
                ledger_declaration_kind=contract.ledger_declaration_kind,
                ledger_statement=contract.ledger_statement,
                tla_proof=contract.tla_proof,
            )
        )
    module.CROSS_TOOL_REFINEMENT_CONTRACTS = tuple(hardened_contracts)
    module.CROSS_TOOL_REFINEMENT_BY_ID = {
        contract.obligation_id: contract
        for contract in module.CROSS_TOOL_REFINEMENT_CONTRACTS
    }

    ledger = complete_cross_tool_ledger(module)
    formal_dir = tmp_path / "docs" / "formal" / "sumeragi_v2"
    shutil.copytree(
        module.FORMAL_DIR,
        formal_dir,
        ignore=shutil.ignore_patterns(".tlacache"),
    )

    contracts_by_module = {}
    for contract in module.CROSS_TOOL_REFINEMENT_CONTRACTS:
        contracts_by_module.setdefault(contract.module, []).append(contract)
    for module_name, contracts in contracts_by_module.items():
        path = formal_dir / f"{module_name}.tla"
        source = path.read_text(encoding="utf-8")
        original_source = source
        inherited_source = "\n".join(
            provider_source
            for _, _, provider_source in module._cross_tool_tla_module_closure(
                formal_dir, module_name
            )
        )
        model_side_declarations = ""
        for contract in contracts:
            premise = contract.tla_statement.split(" => ", maxsplit=1)[0]
            if module._expanded_tla_alias(
                inherited_source, premise
            ) == module._expanded_tla_alias(
                inherited_source, contract.ledger_symbol
            ):
                synthetic = f"{contract.tla_theorem}SyntheticModelSide"
                old = f"THEOREM {contract.ledger_symbol} ==\n  {premise}"
                assert source.count(old) == 1
                source = source.replace(
                    old,
                    f"THEOREM {contract.ledger_symbol} ==\n"
                    f"  /\\ {premise}\n"
                    f"  /\\ {synthetic}",
                    1,
                )
                model_side_declarations += f"\n{synthetic} == FALSE\n"
        end = source.rfind("====")
        assert end >= 0
        declarations = model_side_declarations + "".join(
            "\nTHEOREM "
            f"{contract.tla_theorem} ==\n"
            f"  {contract.tla_statement}\n"
            "PROOF\n"
            "  OBVIOUS\n"
            for contract in contracts
            if module._top_level_theorem_body(
                inherited_source, contract.tla_theorem
            )
            is None
        )
        if source != original_source or declarations:
            path.write_text(
                source[:end] + declarations + "\n====\n",
                encoding="utf-8",
            )

    verus_contract = module._verus_evidence_contract_module()
    production_sources = {
        relative
        for contract in module.CROSS_TOOL_REFINEMENT_CONTRACTS
        for claim in contract.claims
        for relative in claim.production_sources
    }
    copied_sources = (
        set(verus_contract.REQUIRED_SOURCE_PATHS)
        | production_sources
        | {"crates/iroha_core/src/sumeragi/v2_core.rs"}
    )
    for relative in sorted(copied_sources):
        destination = tmp_path / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        source = ROOT_DIR / relative
        if source.is_file():
            shutil.copyfile(source, destination)
        else:
            # The fixture exercises the evidence schema independently of
            # unrelated source-inventory migrations in the shared worktree.
            destination.write_text("// synthetic fixture source\n", encoding="utf-8")

    theorem_claims_by_source = {}
    for contract in module.CROSS_TOOL_REFINEMENT_CONTRACTS:
        for claim in contract.claims:
            theorem_claims_by_source.setdefault(claim.verus_source, []).append(
                claim
            )
    for relative, claims in theorem_claims_by_source.items():
        path = tmp_path / relative
        legacy_claims = [
            claim
            for claim in claims
            if claim.proof_mode == "legacy_requires_builder"
        ]
        if not legacy_claims:
            continue
        assert len(legacy_claims) == len(claims)
        source = ""
        synthetic_proofs = "\nverus! {\n"
        for claim in legacy_claims:
            expected_call = (
                f"{claim.verified_kernel}({claim.theorem_kernel_projection})"
            )
            synthetic_proofs += (
                f"pub closed spec fn {claim.theorem_projection_builder}("
                f"{claim.theorem_projection_builder_parameters}) -> "
                f"{claim.theorem_projection_builder_return} {{\n"
                "    projection\n"
                "}\n"
                f"pub closed spec fn {claim.verified_kernel}("
                f"{claim.verified_kernel_parameters}) -> bool {{\n"
                f"    {claim.verified_kernel_body}\n"
                "}\n"
                f"pub proof fn {claim.verus_theorem}({claim.verus_parameters})\n"
                f"    requires {claim.verus_requires},\n"
                f"    ensures {claim.verus_ensures},\n"
                "{\n"
                f"    assert({expected_call});\n"
                "}\n"
            )
        synthetic_proofs += "}\n"
        path.write_text(source + synthetic_proofs, encoding="utf-8")

    kernel_path = tmp_path / shared_kernel_source
    kernel_source = kernel_path.read_text(encoding="utf-8")
    kernel_source += "\n" + "".join(
        f"pub(crate) const fn {claim.verified_kernel}"
        f"({claim.verified_kernel_parameters}) -> bool {{\n"
        f"    {claim.verified_kernel_body}\n"
        "}\n"
        for contract in module.CROSS_TOOL_REFINEMENT_CONTRACTS
        for claim in contract.claims
        if claim.proof_mode == "legacy_requires_builder"
    )
    kernel_path.write_text(kernel_source, encoding="utf-8")

    for contract in module.CROSS_TOOL_REFINEMENT_CONTRACTS:
        for claim in contract.claims:
            if claim.proof_mode != "legacy_requires_builder":
                continue
            for call_site in claim.production_call_sites:
                path = tmp_path / call_site.source
                source = path.read_text(encoding="utf-8")
                source += (
                    "\n"
                    f"fn {call_site.item}(projection: u64) {{\n"
                    f"    {call_site.required_expression}\n"
                    "}\n"
                )
                path.write_text(source, encoding="utf-8")

    # Cross-tool release evidence must describe the exact ledger that is part
    # of the source-bound checkout, not a separately supplied archive mutant.
    (formal_dir / "proof_coverage.json").write_text(
        json.dumps(ledger, indent=2) + "\n",
        encoding="utf-8",
    )

    log_dir = tmp_path / module.FORMAL_EVIDENCE_LOGICAL_ROOT / "tlaps"
    write_tlaps_fixture_logs(module, formal_dir, tmp_path, log_dir)
    tlaps_evidence = module.build_release_evidence(
        tlapm_version=module.TLAPM_COMMIT[:7],
        log_dir=log_dir,
        formal_dir=formal_dir,
        root_dir=tmp_path,
    )

    host = verus_contract._host_key()
    if host not in verus_contract.EXPECTED_TOOL_SHA256:
        pytest.skip(f"cross-tool evidence fixture has no pinned Verus host {host}")
    pinned_tool = verus_contract.EXPECTED_TOOL_SHA256[host]
    workspace_manifest_sha256 = "a" * 64
    nonce = "b" * 64
    verus_log = tmp_path / verus_contract.EXPECTED_LOG_PATH
    verus_log.parent.mkdir(parents=True, exist_ok=True)
    verus_log.write_text(
        verus_contract.begin_marker(nonce, workspace_manifest_sha256)
        + "\n"
        + "verification results:: "
        + f"{verus_contract.EXPECTED_DEPENDENCY_VERIFIED} verified, 0 errors\n"
        + "verification results:: "
        + f"{verus_contract.EXPECTED_ROOT_VERIFIED} verified, 0 errors\n"
        + verus_contract.success_marker(nonce, workspace_manifest_sha256)
        + "\n",
        encoding="utf-8",
    )
    verus_evidence = {
        "schema_version": verus_contract.SCHEMA_VERSION,
        "verification_contract_sha256": verus_contract.verification_contract_sha256(),
        "source_manifest_sha256": workspace_manifest_sha256,
        "sources": verus_contract._source_entries(tmp_path),
        "tool": {
            "version": verus_contract.EXPECTED_VERUS_VERSION,
            "platform": pinned_tool["platform"],
            "verus_sha256": pinned_tool["verus"],
            "cargo_verus_sha256": pinned_tool["cargo_verus"],
        },
        "invocation": list(verus_contract.EXPECTED_INVOCATION),
        "log": verus_contract.EXPECTED_LOG_PATH,
        "log_sha256": module._sha256_file(verus_log),
        "nonce": nonce,
        "results": {
            "dependency_verified": verus_contract.EXPECTED_DEPENDENCY_VERIFIED,
            "root_verified": verus_contract.EXPECTED_ROOT_VERIFIED,
            "errors": 0,
        },
        "backend_verification": True,
    }
    cross_tool_evidence = module.build_cross_tool_evidence(
        ledger,
        tlaps_evidence=tlaps_evidence,
        verus_evidence=verus_evidence,
        formal_dir=formal_dir,
        root_dir=tmp_path,
        expected_verus_source_manifest_sha256=workspace_manifest_sha256,
    )
    return (
        ledger,
        formal_dir,
        tlaps_evidence,
        verus_evidence,
        cross_tool_evidence,
        workspace_manifest_sha256,
    )

RELEASE_RECEIPT_COMPONENT_FILES = (
    Path("scripts/write_sumeragi_v2_release_receipt_formal_artifacts.py"),
    Path("scripts/write_sumeragi_v2_release_receipt_corridor_log.py"),
    Path("scripts/write_sumeragi_v2_release_receipt_gate_evidence.py"),
    Path("scripts/write_sumeragi_v2_release_receipt_publication.py"),
)
RELEASE_BOOTSTRAP_COMPONENT_FILES = (
    Path("scripts/bootstrap_sumeragi_v2_release_receipt_replay.py"),
)


def _release_inventory_fixture_paths(module, paths: tuple[Path, ...]) -> tuple[Path, ...]:
    """Expand reviewed Rust parents to their exact include-component closure."""

    reviewed_paths = [
        Path("ci/run_native_amx_v2_grouped_sdk_parity.sh"),
        Path("ci/run_sumeragi_v2_sdk_diagnostics.sh"),
        Path("ci/check_sumeragi_v2_multilane_release_inventory.sh"),
        Path("javascript/iroha_js/test/sumeragiDiagnosticsContract.test.js"),
        Path("javascript/iroha_js/test/toriiClient.test.js"),
        Path("crates/iroha_core/src/kura/autonomous_retired_attempt.rs"),
        Path(
            "crates/iroha_core/src/sumeragi/v2_worker/"
            "autonomous_lane_output_reconstruction.rs"
        ),
        Path("crates/iroha_core/src/sumeragi/v2_runner_tests.rs"),
        Path("specs/sumeragi_v2_multilane_closure_ledger.md"),
        *(Path(relative) for relative in module._READY_VALIDATE_WAL_CRASH_SOURCE_FILES),
        *paths,
    ]
    expanded: list[Path] = []

    def append_closure(relative: Path) -> None:
        if relative in expanded:
            return
        expanded.append(relative)
        for component in module._REVIEWED_RUST_INCLUDE_MANIFESTS.get(
            relative.as_posix(), ()
        ):
            append_closure(relative.parent / component)
        if relative == Path("scripts/write_sumeragi_v2_release_receipt.py"):
            for component in RELEASE_RECEIPT_COMPONENT_FILES:
                append_closure(component)
        if relative == Path("scripts/bootstrap_sumeragi_v2_release.py"):
            for component in RELEASE_BOOTSTRAP_COMPONENT_FILES:
                append_closure(component)

    for relative in reviewed_paths:
        append_closure(relative)
    return tuple(expanded)


@pytest.mark.parametrize(
    ("old", "new", "expected_error"),
    (
        (
            "  peer::shared_byte_budget_tests::frame_retention_coalesces_each_distinct_source_owner_without_reaccounting\n",
            "",
            "must contain exactly 881 tests",
        ),
        (
            "  peer::shared_byte_budget_tests::frame_retention_coalesces_each_distinct_source_owner_without_reaccounting\n",
            "  peer::shared_byte_budget_tests::authenticated_source_count_registry_bounds_identity_churn_and_capacity_drift\n",
            "production liveness inventory repeats tests",
        ),
        *(
            (
                f"  {test_name}\n",
                "",
                f"production ownership regression {test_name} must be pinned exactly once; found 0",
            )
            for test_name in (
                "sumeragi::v2_effects::tests::exact_candidate_retry_coalesces_under_the_incumbent_owner",
                "sumeragi::v2_effects::tests::fetch_owner_replacement_is_rejected_before_upgrade_refinement_or_request_work",
                "sumeragi::v2_effects::tests::adapter_effect_retry_policy_is_closed_over_all_eleven_effect_classes",
                "sumeragi::v2_lifecycle_coordinator::launch::tests::recovered_decision_fetch_composite_dispatch_reserves_capacity_before_claim_and_commit",
                "sumeragi::v2_lane_work::tests::native_amx_manifest_projects_finality_bound_merge_batch_in_canonical_order",
                "sumeragi::v2_lane_work::tests::native_amx_merge_projection_rejects_multiple_participant_heights_in_one_carrier",
                "sumeragi::v2_lane_work::tests::native_amx_merge_projection_rejects_same_height_participant_identity_conflict",
                "sumeragi::v2_lane_work::tests::native_amx_merge_projection_excludes_coordinator_only_receipts",
                "sumeragi::v2_lane_work::tests::native_amx_merge_projection_rejects_same_route_identity_conflict",
                "sumeragi::v2_lane_work::tests::native_amx_merge_projection_rejects_duplicate_group_source",
                "sumeragi::v2_lane_work::tests::native_amx_merge_projection_matches_decoded_replay_entry",
                "sumeragi::v2_runtime::tests::adapter_effect_binding_is_exact_route_neutral_and_three_bounded",
                "sumeragi::v2_runtime::tests::certified_body_pipeline_retains_statement_and_owner_across_stage_kinds",
                "sumeragi::v2_runtime::tests::body_pipeline_acquires_commit_authority_monotonically_under_one_owner",
                "sumeragi::v2_runtime::tests::pending_validate_projects_exact_prepare_commit_and_report_successors",
                "sumeragi::v2_runtime::tests::pending_validate_projects_only_the_exact_commit_authorized_apply_successor",
                "sumeragi::v2_runtime::tests::drained_internal_ignore_uses_exact_durable_tombstone_before_readmission",
                "sumeragi::v2_runtime::tests::queued_body_completion_coalesces_only_its_incumbent_owner",
                "sumeragi::v2_runtime::tests::stale_internal_callback_is_marker_free_and_malformed_callback_spends_no_ordinal",
                "sumeragi::v2_lifecycle_coordinator::tests::restart_seeds_high_water_and_rollover_preserves_it",
                "sumeragi::v2_lifecycle_coordinator::tests::producer_handoff_blocks_later_work_without_making_serve_a_global_barrier",
                "sumeragi::v2_certified_serve_payload_store::tests::completed_payload_requires_exact_certified_responder_authority",
                "sumeragi::v2_certified_serve_payload_store::tests::production_open_consumes_the_exact_kura_directory_authority",
                "sumeragi::v2_certified_serve_payload_store::tests::emergency_fast_payload_store_skips_inventory_and_rejects_retirement",
                "sumeragi::v2_lifecycle_coordinator::ledger::tests::durable_ready_fetch_recovery::complete_tip_corrupt_payload_rejects_before_live_apply_ledger_repair",
                "sumeragi::v2::tests::ready_local_proposal_sign_and_exact_output_precede_pending_timeout_certificate",
                "state::tests::block_leaves_governance_unlock_audit_clean_when_no_locks_are_expired",
                "queue::tests::replica_disposition_observes_exact_fifo_beneath_global_selection_overlay",
            )
        ),
        *(
            (
                "  peer::shared_byte_budget_tests::frame_retention_coalesces_each_distinct_source_owner_without_reaccounting\n",
                f"  sumeragi::v2_core::network_simulation::{test_name}\n",
                "must map to exactly one reviewed module",
            )
            for test_name in (
                "lossy_offline_leader_simulations_commit_for_4_7_and_10_validators",
                "two_by_two_partition_cannot_advance_but_healing_retransmits_tc_and_commits",
                "historical_prepare_qc_uses_current_consumer_tag_after_timeout_install",
                "responsive_source_redelivers_exact_prepare_qc_after_lagger_installs_tc",
                "asymmetric_partition_stalls_without_dual_quorum_then_heals_and_applies",
                "leader_crash_after_proposal_broadcast_does_not_block_the_remaining_quorum",
                "leader_crash_with_a_locked_body_rotates_and_rebuilds_the_old_commit_quorum",
                "corrupted_chunks_and_withheld_commit_evidence_recover_by_bounded_retransmission",
                "crash_after_proposal_wal_before_signature_replays_exact_intent",
                "divergent_views_converge_and_commit_within_one_rotation",
            )
        ),
        (
            "  sumeragi::v2_runner::tests::"
            "terminal_sweep_source_partitions_whole_units_before_any_mutation\n",
            "  sumeragi::v2_runner::tests::"
            "terminal_sweep_source_partitions_whole_units_before_any_mutation_mutant\n",
            "canonical module/test inventory SHA-256",
        ),
        (
            "readonly expected_production_liveness_test_count=881",
            "readonly expected_production_liveness_test_count=861",
            "production liveness source count must be sealed as 881",
        ),
        (
            "  sumeragi::v2_core::tests\n"
            "  sumeragi::v2_core::refinement::tests\n",
            "  sumeragi::v2_core::tests\n"
            "  sumeragi::v2_core::network_simulation\n"
            "  sumeragi::v2_core::refinement::tests\n",
            "production liveness modules must equal the reviewed ordered",
        ),
        (
            "  production-v2-core\n"
            "  production-v2-core-refinement\n",
            "  production-v2-core\n"
            "  production-v2-core-network-simulation\n"
            "  production-v2-core-refinement\n",
            "production module leg IDs must equal the reviewed",
        ),
        (
            "readonly expected_typed_rollover_formal_mutation_count=45",
            "readonly expected_typed_rollover_formal_mutation_count=44",
            "45-mutation typed rollover contract fragment",
        ),
        (
            "(INVARIANT|TEMPORAL)_MARKER",
            "INVARIANT_MARKER",
            "45-mutation typed rollover contract fragment",
        ),
        (
            'echo "[tlc] typed rollover-handoff repaired models and 45-mutant '
            'root-anchored V3 matrix passed"',
            'echo "[tlc] typed rollover-handoff matrix passed"',
            "45-mutation typed rollover contract fragment",
        ),
        (
            "readonly expected_multilane_focus_test_count=531",
            "readonly expected_multilane_focus_test_count=530",
            "multilane G-UNIT source count must be sealed as 531",
        ),
        (
            '  if [[ "$(wc -l <"$corridor_g_unit_inventory" | tr -d '
                """'[:space:]')" != 532 ]]; then""",
            '  if [[ "$(wc -l <"$corridor_g_unit_inventory" | tr -d '
                """'[:space:]')" != 531 ]]; then""",
            "G-UNIT TSV guard must require one header plus exactly 531 focus rows",
        ),
        (
            "The canonical 531-row TSV is",
            "The canonical 530-row TSV is",
            "G-UNIT inventory comment must seal 531 rows",
        ),
        (
            "including exact 531/531 G-UNIT,",
            "including exact 530/531 G-UNIT,",
            "terminal success text must seal exact 531/531 G-UNIT",
        ),
        (
            "  kura::tests::native_amx_prevote_byte_budget_is_exact_per_route_and_finality_width_stable\n",
            "  kura::tests::native_amx_prevote_byte_budget_is_exact_per_route_and_finality_width_stable_mutant\n",
            "canonical G-UNIT leg/crate/test inventory SHA-256",
        ),
        (
            "  kura::tests::native_amx_prevote_pair_geometry_rejects_empty_hard_cap_and_overflow\n",
            "  kura::tests::native_amx_prevote_pair_geometry_rejects_empty_hard_cap_and_overflow_mutant\n",
            "canonical G-UNIT leg/crate/test inventory SHA-256",
        ),
        (
            "  sumeragi::v2_apply::tests::native_amx_prevote_byte_failures_have_precommit_error_classification\n",
            "  sumeragi::v2_apply::tests::native_amx_prevote_byte_failures_have_precommit_error_classification_mutant\n",
            "canonical G-UNIT leg/crate/test inventory SHA-256",
        ),
        (
            "  sumeragi::v2_core::refinement::tests::"
            "in_flight_reservation_kernel_accepts_only_identity_bound_local_owner_steps\n",
            "  sumeragi::v2_core::refinement::tests::"
            "in_flight_reservation_kernel_accepts_only_identity_bound_local_owner_steps_mutant\n",
            "canonical G-UNIT leg/crate/test inventory SHA-256",
        ),
        (
            "  queue::reservation_journal::tests::"
            "post_sync_append_publication_failure_is_poisoned_and_replayed_on_reopen\n",
            "  queue::reservation_journal::tests::"
            "post_sync_append_publication_failure_is_poisoned_and_replayed_on_reopen_mutant\n",
            "canonical G-UNIT leg/crate/test inventory SHA-256",
        ),
        (
            "  queue::reservation_journal::tests::"
            "post_sync_compaction_publication_failure_is_poisoned_and_replayed_on_reopen\n",
            "  queue::reservation_journal::tests::"
            "post_sync_compaction_publication_failure_is_poisoned_and_replayed_on_reopen_mutant\n",
            "canonical G-UNIT leg/crate/test inventory SHA-256",
        ),
        (
            "  queue::reservation_journal::tests::"
            "runtime_commit_requires_live_owner_but_snapshot_recovery_may_restore_commit_barrier\n",
            "  queue::reservation_journal::tests::"
            "runtime_commit_requires_live_owner_but_snapshot_recovery_may_restore_commit_barrier_mutant\n",
            "canonical G-UNIT leg/crate/test inventory SHA-256",
        ),
        (
            "  queue::reservation_journal::tests::"
            "prepared_checked_transition_is_bound_to_frame_and_state_generation\n",
            "  queue::reservation_journal::tests::"
            "prepared_checked_transition_is_bound_to_frame_and_state_generation_mutant\n",
            "canonical G-UNIT leg/crate/test inventory SHA-256",
        ),
        (
            "  queue::reservation_journal::tests::"
            "prepared_checked_transition_rejects_same_generation_cross_state_substitution\n",
            "  queue::reservation_journal::tests::"
            "prepared_checked_transition_rejects_same_generation_cross_state_substitution_mutant\n",
            "canonical G-UNIT leg/crate/test inventory SHA-256",
        ),
        (
            "  queue::reservation_journal::tests::"
            "prepared_checked_transition_binds_exact_ordered_owner_token_coverage\n",
            "  queue::reservation_journal::tests::"
            "prepared_checked_transition_binds_exact_ordered_owner_token_coverage_mutant\n",
            "canonical G-UNIT leg/crate/test inventory SHA-256",
        ),
        (
            "  queue::reservation_journal::tests::"
            "checked_transition_result_identity_and_candidate_application_are_atomic\n",
            "  queue::reservation_journal::tests::"
            "checked_transition_result_identity_and_candidate_application_are_atomic_mutant\n",
            "canonical G-UNIT leg/crate/test inventory SHA-256",
        ),
        (
            "  queue::reservation_journal::tests::"
            "checked_transition_generation_overflow_is_rejected_without_mutation\n",
            "  queue::reservation_journal::tests::"
            "checked_transition_generation_overflow_is_rejected_without_mutation_mutant\n",
            "canonical G-UNIT leg/crate/test inventory SHA-256",
        ),
        (
            "  queue::reservation_journal::tests::"
            "snapshot_replay_seal_covers_empty_and_live_owner_replays\n",
            "  queue::reservation_journal::tests::"
            "snapshot_replay_seal_covers_empty_and_live_owner_replays_mutant\n",
            "canonical G-UNIT leg/crate/test inventory SHA-256",
        ),
        (
            "  queue::reservation_journal::tests::"
            "snapshot_replay_seal_rejects_changed_journal_before_publication\n",
            "  queue::reservation_journal::tests::"
            "snapshot_replay_seal_rejects_changed_journal_before_publication_mutant\n",
            "canonical G-UNIT leg/crate/test inventory SHA-256",
        ),
        (
            "  queue::reservation_journal::tests::"
            "snapshot_replay_receipt_rejects_same_count_owner_identity_drift\n",
            "  queue::reservation_journal::tests::"
            "snapshot_replay_receipt_rejects_same_count_owner_identity_drift_mutant\n",
            "canonical G-UNIT leg/crate/test inventory SHA-256",
        ),
        (
            "  native_amx::tests::signing_guard_durably_binds_full_source_session_and_participant_incarnation\n"
            "  native_amx::tests::signing_guard_durable_commit_rejects_conflicting_later_prepares_across_restart\n",
            "  native_amx::tests::signing_guard_durable_commit_rejects_conflicting_later_prepares_across_restart\n"
            "  native_amx::tests::signing_guard_durably_binds_full_source_session_and_participant_incarnation\n",
            "canonical G-UNIT leg/crate/test inventory SHA-256",
        ),
        (
            "  block::tests::historical_native_amx_source_bundle_"
            "authenticates_every_evidence_layer\n",
            "  block::tests::historical_native_amx_source_bundle_"
            "authenticates_every_evidence_layer_mutant\n",
            "canonical G-UNIT leg/crate/test inventory SHA-256",
        ),
        (
            "  kura::tests::native_amx_all_manifest_barrier_"
            "does_not_promote_another_routes_receipt_temp\n",
            "  kura::tests::native_amx_all_manifest_barrier_"
            "does_not_promote_another_routes_receipt_temp_mutant\n",
            "canonical G-UNIT leg/crate/test inventory SHA-256",
        ),
        (
            "  sumeragi::v2_apply::tests::historical_autonomous_recovery_"
            "reaches_exactly_once_canonical_merge_application\n",
            "  sumeragi::v2_apply::tests::historical_autonomous_recovery_"
            "reaches_exactly_once_canonical_merge_application_mutant\n",
            "canonical G-UNIT leg/crate/test inventory SHA-256",
        ),
        (
            "  append_g_unit_inventory \\\n"
            '    g-unit-iroha-core iroha_core "${required_multilane_core_focus_tests[@]}"',
            "  append_g_unit_inventory \\\n"
            '    g-unit-iroha-core iroha_p2p "${required_multilane_core_focus_tests[@]}"',
            "G-UNIT leg g-unit-iroha-core must append the exact",
        ),
        (
            "  block::consensus_v2::finality::tests::header_binding_allows_unchanged_reproposal_but_rejects_earlier_decision_round\n"
            "  block::consensus_v2::tests::kagemusha_consensus_signature_envelope_roundtrips_and_rejects_drift\n",
            "  block::consensus_v2::tests::kagemusha_consensus_signature_envelope_roundtrips_and_rejects_drift\n"
            "  block::consensus_v2::finality::tests::header_binding_allows_unchanged_reproposal_but_rejects_earlier_decision_round\n",
            "canonical module/test inventory SHA-256",
        ),
        (
            'production_p2p_unit_list="$(run_cargo test --locked --offline -p iroha_p2p --lib -- --list)"',
            'production_p2p_unit_list="$(run_cargo test --locked --offline -p iroha_p2p --all-features --lib -- --list)"',
            "reviewed P2P corridor must use exact default-feature test discovery",
        ),
        (
            'production_config_unit_list="$(run_cargo test --locked --offline -p iroha_config --lib -- --list)"',
            'production_config_unit_list="$(run_cargo test --locked --offline -p iroha_config --all-features --lib -- --list)"',
            "exact-output configuration discovery must use the exact iroha_config library test surface",
        ),
        (
            'elif [[ "$required_test" == parameters::* ]]; then',
            'elif [[ "$required_test" == configuration::* ]]; then',
            "exact-output configuration tests must route through the iroha_config library corridor",
        ),
        (
            'elif [[ "$module" == parameters::* ]]; then\n'
            '    module_command="cargo test --locked --offline -p iroha_config --lib '
            '${module} -- --test-threads=1"',
            'elif [[ "$module" == parameters::* ]]; then\n'
            '    module_command="cargo test --locked --offline -p iroha_core --lib '
            '${module} -- --test-threads=1"',
            "exact-output configuration tests must route through the iroha_config library corridor",
        ),
    ),
)
def test_production_release_inventory_rejects_name_count_and_feature_mutants(
    tmp_path: Path,
    old: str,
    new: str,
    expected_error: str,
) -> None:
    module = load_checker()
    for relative in _release_inventory_fixture_paths(
        module,
        (
            Path("scripts/run_sumeragi_v2_release_gates.sh"),
            Path("scripts/write_sumeragi_v2_release_receipt.py"),
            Path("scripts/bootstrap_sumeragi_v2_release.py"),
            Path("scripts/validate_sumeragi_v2_release_bootstrap.py"),
            Path("formal/sumeragi_v2/README.md"),
            Path("formal/sumeragi_v2/PROOF.md"),
            Path("specs/sumeragi_v2_liveness.md"),
            Path("scripts/bootstrap_sumeragi_v2_release.py"),
            Path("scripts/validate_sumeragi_v2_release_bootstrap.py"),
            Path("crates/iroha_data_model/src/block/consensus_v2/finality.rs"),
            Path("integration_tests/tests/sumeragi_v2_runner.rs"),
            Path("crates/iroha_core/src/sumeragi/v2.rs"),
            Path("crates/iroha_core/src/sumeragi/v2_runner.rs"),
        ),
    ):
        destination = tmp_path / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(ROOT_DIR / relative, destination)

    release_path = tmp_path / "scripts" / "run_sumeragi_v2_release_gates.sh"
    source = release_path.read_text(encoding="utf-8")
    assert source.count(old) == 1, old
    release_path.write_text(source.replace(old, new, 1), encoding="utf-8")

    errors = module._production_liveness_release_inventory_errors(tmp_path)
    assert any(expected_error in error for error in errors), errors


@pytest.mark.parametrize(
    ("relative", "old", "new", "expected_error"),
    (
        (
            Path("scripts/run_sumeragi_v2_release_gates.sh"),
            "  native_amx_grouped_parity_test_counts=(\n"
            "    7\n"
            "    67\n"
            "    65\n",
            "  native_amx_grouped_parity_test_counts=(\n"
            "    7\n"
            "    66\n"
            "    65\n",
            "grouped Native AMX SDK runner suite inventory must equal",
        ),
        (
            Path("ci/run_native_amx_v2_grouped_sdk_parity.sh"),
            "  python)\n    observed_test_count=67\n",
            "  python)\n    observed_test_count=66\n",
            "grouped Native AMX SDK harness suite inventory must equal",
        ),
        (
            Path("scripts/write_sumeragi_v2_release_receipt.py"),
            '    ("python", 67),\n',
            '    ("python", 66),\n',
            "grouped Native AMX SDK receipt suite inventory must equal",
        ),
    ),
)
def test_production_release_inventory_rejects_grouped_sdk_count_drift(
    tmp_path: Path,
    relative: Path,
    old: str,
    new: str,
    expected_error: str,
) -> None:
    module = load_checker()
    fixture_paths = _release_inventory_fixture_paths(
        module,
        (
            Path("scripts/run_sumeragi_v2_release_gates.sh"),
            Path("scripts/write_sumeragi_v2_release_receipt.py"),
            Path("scripts/bootstrap_sumeragi_v2_release.py"),
            Path("scripts/validate_sumeragi_v2_release_bootstrap.py"),
            Path("formal/sumeragi_v2/README.md"),
            Path("formal/sumeragi_v2/PROOF.md"),
            Path("specs/sumeragi_v2_liveness.md"),
            Path("crates/iroha_data_model/src/block/consensus_v2/finality.rs"),
            Path("integration_tests/tests/sumeragi_v2_runner.rs"),
            Path("crates/iroha_core/src/sumeragi/v2.rs"),
            Path("crates/iroha_core/src/sumeragi/v2_runner.rs"),
            Path("crates/iroha_core/src/sumeragi/v2_lane_work.rs"),
            Path("ci/run_native_amx_v2_grouped_sdk_parity.sh"),
        ),
    )
    for fixture_relative in fixture_paths:
        destination = tmp_path / fixture_relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(ROOT_DIR / fixture_relative, destination)

    baseline_errors = module._production_liveness_release_inventory_errors(tmp_path)
    assert baseline_errors == [], baseline_errors
    target = tmp_path / relative
    source = target.read_text(encoding="utf-8")
    assert source.count(old) == 1, old
    target.write_text(source.replace(old, new, 1), encoding="utf-8")

    errors = module._production_liveness_release_inventory_errors(tmp_path)
    assert any(expected_error in error for error in errors), errors


@pytest.mark.parametrize(
    ("relative", "old", "new", "expected_error"),
    (
        (
            Path("scripts/run_sumeragi_v2_release_gates.sh"),
            "  sumeragi_v2_sdk_diagnostics_test_counts=(\n"
            "    129\n"
            "    90\n",
            "  sumeragi_v2_sdk_diagnostics_test_counts=(\n"
            "    125\n"
            "    90\n",
            "Sumeragi SDK diagnostics runner suite inventory must equal",
        ),
        (
            Path("ci/run_sumeragi_v2_sdk_diagnostics.sh"),
            "  python)\n    observed_test_count=129\n",
            "  python)\n    observed_test_count=125\n",
            "Sumeragi SDK diagnostics harness suite inventory must equal",
        ),
        (
            Path("scripts/write_sumeragi_v2_release_receipt.py"),
            '    ("python", 129),\n',
            '    ("python", 125),\n',
            "Sumeragi SDK diagnostics receipt suite inventory must equal",
        ),
        (
            Path("javascript/iroha_js/test/sumeragiDiagnosticsContract.test.js"),
            '  "typed Sumeragi endpoints reject swapped status and diagnostics payloads",\n',
            "",
            "dedicated JavaScript Sumeragi diagnostics inventory must contain exactly 45",
        ),
        (
            Path("ci/run_sumeragi_v2_sdk_diagnostics.sh"),
            '    "# skipped": 0,\n',
            '    "# skipped": 1,\n',
            "no-skip selector lacks exact fragment",
        ),
        (
            Path("scripts/run_sumeragi_v2_release_gates.sh"),
            "# Execute every maintained consumer of the Rust-owned grouped Native AMX V2\n",
            "# --test-name-pattern is retired; execute every maintained consumer.\n",
            "retains retired ordinal/partial selector '--test-name-pattern'",
        ),
    ),
)
def test_production_release_inventory_rejects_sdk_diagnostics_drift(
    tmp_path: Path,
    relative: Path,
    old: str,
    new: str,
    expected_error: str,
) -> None:
    module = load_checker()
    fixture_paths = _release_inventory_fixture_paths(
        module,
        (
            Path("scripts/run_sumeragi_v2_release_gates.sh"),
            Path("scripts/write_sumeragi_v2_release_receipt.py"),
            Path("scripts/bootstrap_sumeragi_v2_release.py"),
            Path("scripts/validate_sumeragi_v2_release_bootstrap.py"),
            Path("formal/sumeragi_v2/README.md"),
            Path("formal/sumeragi_v2/PROOF.md"),
            Path("specs/sumeragi_v2_liveness.md"),
            Path("crates/iroha_data_model/src/block/consensus_v2/finality.rs"),
            Path("integration_tests/tests/sumeragi_v2_runner.rs"),
            Path("crates/iroha_core/src/sumeragi/v2.rs"),
            Path("crates/iroha_core/src/sumeragi/v2_runner.rs"),
            Path("crates/iroha_core/src/sumeragi/v2_lane_work.rs"),
        ),
    )
    for fixture_relative in fixture_paths:
        destination = tmp_path / fixture_relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(ROOT_DIR / fixture_relative, destination)

    baseline_errors = module._production_liveness_release_inventory_errors(tmp_path)
    assert baseline_errors == [], baseline_errors
    target = tmp_path / relative
    source = target.read_text(encoding="utf-8")
    assert source.count(old) == 1, old
    target.write_text(source.replace(old, new, 1), encoding="utf-8")

    errors = module._production_liveness_release_inventory_errors(tmp_path)
    assert any(expected_error in error for error in errors), errors


def test_production_release_inventory_seals_later_genesis_proposal_origin(
    tmp_path: Path,
) -> None:
    module = load_checker()
    required_paths = (
        Path("scripts/run_sumeragi_v2_release_gates.sh"),
        Path("ci/check_sumeragi_v2_multilane_release_inventory.sh"),
        Path("scripts/write_sumeragi_v2_release_receipt.py"),
        Path("scripts/bootstrap_sumeragi_v2_release.py"),
        Path("scripts/validate_sumeragi_v2_release_bootstrap.py"),
        Path("formal/sumeragi_v2/README.md"),
        Path("formal/sumeragi_v2/PROOF.md"),
        Path("specs/sumeragi_v2_liveness.md"),
        Path("scripts/bootstrap_sumeragi_v2_release.py"),
        Path("scripts/validate_sumeragi_v2_release_bootstrap.py"),
        Path("crates/iroha_data_model/src/block/consensus_v2/finality.rs"),
        Path("integration_tests/tests/sumeragi_v2_runner.rs"),
        Path("crates/iroha_core/src/sumeragi/v2.rs"),
        Path("crates/iroha_core/src/sumeragi/v2_runner.rs"),
        Path("crates/iroha_core/src/sumeragi/v2_lane_work.rs"),
    )
    required_paths = _release_inventory_fixture_paths(module, required_paths)
    for relative in required_paths:
        destination = tmp_path / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(ROOT_DIR / relative, destination)

    errors = module._production_liveness_release_inventory_errors(tmp_path)
    assert errors == [], errors

    finality_path = (
        tmp_path
        / "crates"
        / "iroha_data_model"
        / "src"
        / "block"
        / "consensus_v2"
        / "finality.rs"
    )
    source = finality_path.read_text(encoding="utf-8")
    exact_call = "artifact_bound_to_header(3, 5)"
    assert source.count(exact_call) == 1
    finality_path.write_text(
        source.replace(exact_call, "artifact_bound_to_header(4, 5)", 1),
        encoding="utf-8",
    )

    errors = module._production_liveness_release_inventory_errors(tmp_path)
    assert any(
        "genesis header-binding release regression must match exact reviewed "
        "token digest" in error
        for error in errors
    ), errors


def test_production_release_inventory_seals_contention_tolerant_restart_deadline(
    tmp_path: Path,
) -> None:
    module = load_checker()
    required_paths = (
        Path("scripts/run_sumeragi_v2_release_gates.sh"),
        Path("ci/check_sumeragi_v2_multilane_release_inventory.sh"),
        Path("scripts/write_sumeragi_v2_release_receipt.py"),
        Path("scripts/bootstrap_sumeragi_v2_release.py"),
        Path("scripts/validate_sumeragi_v2_release_bootstrap.py"),
        Path("formal/sumeragi_v2/README.md"),
        Path("formal/sumeragi_v2/PROOF.md"),
        Path("specs/sumeragi_v2_liveness.md"),
        Path("scripts/bootstrap_sumeragi_v2_release.py"),
        Path("scripts/validate_sumeragi_v2_release_bootstrap.py"),
        Path("crates/iroha_data_model/src/block/consensus_v2/finality.rs"),
        Path("integration_tests/tests/sumeragi_v2_runner.rs"),
        Path("crates/iroha_core/src/sumeragi/v2.rs"),
        Path("crates/iroha_core/src/sumeragi/v2_runner.rs"),
        Path("crates/iroha_core/src/sumeragi/v2_lane_work.rs"),
    )
    required_paths = _release_inventory_fixture_paths(module, required_paths)
    for relative in required_paths:
        destination = tmp_path / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(ROOT_DIR / relative, destination)

    assert module._production_liveness_release_inventory_errors(tmp_path) == []

    runner_path = (
        tmp_path
        / "integration_tests"
        / "tests"
        / "sumeragi_v2_runner"
        / "restart_timing_test.rs"
    )
    source = runner_path.read_text(encoding="utf-8")
    exact_assertion = "assert_eq!(base_round_timeout_ms, 20_000);"
    assert source.count(exact_assertion) == 1
    runner_path.write_text(
        source.replace(
            exact_assertion,
            "assert_eq!(base_round_timeout_ms, 19_999);",
            1,
        ),
        encoding="utf-8",
    )

    errors = module._production_liveness_release_inventory_errors(tmp_path)
    assert any(
        "contention-tolerant restart release regression must match exact "
        "reviewed token digest" in error
        for error in errors
    ), errors


def test_production_release_inventory_seals_successor_parent_binding(
    tmp_path: Path,
) -> None:
    module = load_checker()
    required_paths = (
        Path("scripts/run_sumeragi_v2_release_gates.sh"),
        Path("ci/check_sumeragi_v2_multilane_release_inventory.sh"),
        Path("scripts/write_sumeragi_v2_release_receipt.py"),
        Path("scripts/bootstrap_sumeragi_v2_release.py"),
        Path("scripts/validate_sumeragi_v2_release_bootstrap.py"),
        Path("formal/sumeragi_v2/README.md"),
        Path("formal/sumeragi_v2/PROOF.md"),
        Path("specs/sumeragi_v2_liveness.md"),
        Path("scripts/bootstrap_sumeragi_v2_release.py"),
        Path("scripts/validate_sumeragi_v2_release_bootstrap.py"),
        Path("crates/iroha_data_model/src/block/consensus_v2/finality.rs"),
        Path("integration_tests/tests/sumeragi_v2_runner.rs"),
        Path("crates/iroha_core/src/sumeragi/v2.rs"),
        Path("crates/iroha_core/src/sumeragi/v2_runner.rs"),
        Path("crates/iroha_core/src/sumeragi/v2_lane_work.rs"),
    )
    required_paths = _release_inventory_fixture_paths(module, required_paths)
    for relative in required_paths:
        destination = tmp_path / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(ROOT_DIR / relative, destination)

    errors = module._production_liveness_release_inventory_errors(tmp_path)
    assert errors == [], errors

    mutations = (
        (
            Path("crates/iroha_core/src/sumeragi/tests/v2_adapter_activation_context.rs"),
            "successor_core_context_preserves_the_parent_certificate_binding",
            "assert_ne!(core_parent.context_id(), context_id(successor_id));",
            "assert_eq!(core_parent.context_id(), context_id(successor_id));",
        ),
        (
            Path(
                "crates/iroha_core/src/sumeragi/tests/"
                "v2_adapter_main_00.rs"
            ),
            "successor_context_requires_the_durable_cryptographic_parent",
            "let admitted = adapter\n        .receive_authenticated(authenticated)",
            "let admitted = adapter\n        .receive_authenticated(proposal)",
        ),
        (
            Path(
                "crates/iroha_core/src/sumeragi/tests/"
                "v2_adapter_main_04.rs"
            ),
            "authentication_rejects_valid_commitment_conflicts_without_mutating_adapter",
            "adapter.authenticate(conflicting_proposal_message),\n"
            "        Err(AdapterError::ConflictingExecutionCommitment)",
            "adapter.authenticate(conflicting_proposal_message),\n"
            "        Err(AdapterError::MissingExecutionCommitment)",
        ),
    )
    for relative, test_name, old, new in mutations:
        source_path = tmp_path / relative
        canonical_source = source_path.read_text(encoding="utf-8")
        assert canonical_source.count(old) == 1, old
        source_path.write_text(
            canonical_source.replace(old, new, 1),
            encoding="utf-8",
        )
        errors = module._production_liveness_release_inventory_errors(tmp_path)
        assert any(
            "successor parent-binding release regression "
            f"{test_name} must match exact reviewed token digest" in error
            for error in errors
        ), errors
        source_path.write_text(canonical_source, encoding="utf-8")

    semantic_mutations = (
        (
            Path(
                "crates/iroha_core/src/sumeragi/tests/"
                "v2_adapter_main_00.rs"
            ),
            "Hash::new(b\"substituted successor execution policy\")",
            "successor.execution_policy_hash",
            "successor authentication must reject execution-policy substitution "
            "against the durable parent context",
        ),
        (
            Path(
                "crates/iroha_core/src/sumeragi/tests/"
                "v2_adapter_main_00.rs"
            ),
            "proposal_subject.payload_hash = Hash::new(&proposal_body);",
            "proposal_subject.payload_hash = Hash::new(b\"unbound parent body\");",
            "successor parent-certificate authentication must use a canonical "
            "payload-bound proposal fixture",
        ),
        (
            Path(
                "crates/iroha_core/src/sumeragi/tests/"
                "v2_adapter_main_04.rs"
            ),
            "&locally_validated_payload,",
            "&[0x88, 2],",
            "execution-commitment conflict authentication must bind the locally "
            "validated canonical payload fixture",
        ),
        (
            Path(
                "crates/iroha_core/src/sumeragi/tests/"
                "v2_adapter_main_04.rs"
            ),
            "encode_payload(&context, proposal_round, proposal_subject, &proposal_body)\n"
            "            .expect(\"encode later-view proposal payload\")",
            "encode_payload(&context, proposal_round, proposal_subject, &[0x83, 3])\n"
            "            .expect(\"encode later-view proposal payload\")",
            "embedded-certificate conflict authentication must bind the "
            "later-view canonical payload fixture",
        ),
    )
    for relative, old, new, expected_error in semantic_mutations:
        source_path = tmp_path / relative
        canonical_source = source_path.read_text(encoding="utf-8")
        assert canonical_source.count(old) == 1, old
        source_path.write_text(
            canonical_source.replace(old, new, 1),
            encoding="utf-8",
        )
        errors = module._production_liveness_release_inventory_errors(tmp_path)
        assert any(expected_error in error for error in errors), errors
        source_path.write_text(canonical_source, encoding="utf-8")

    helper_path = (
        tmp_path
        / "crates"
        / "iroha_core"
        / "src"
        / "sumeragi"
        / "v2_worker"
        / "autonomous_lane_output_reconstruction.rs"
    )
    canonical_helper = helper_path.read_text(encoding="utf-8")
    exact_retirement_gate = "bound_supersession_source.is_none()"
    assert canonical_helper.count(exact_retirement_gate) == 1
    helper_path.write_text(
        canonical_helper.replace(
            exact_retirement_gate,
            "bound_supersession_source.is_some()",
            1,
        ),
        encoding="utf-8",
    )
    errors = module._production_liveness_release_inventory_errors(tmp_path)
    assert any(
        "production liveness helper "
        "autonomous_lane_output_has_exact_retirement_source declaration and "
        "complete control flow must match the exact reviewed token digest"
        in error
        for error in errors
    ), errors


def test_production_release_inventory_seals_closed_prefix_suffix_retry(
    tmp_path: Path,
) -> None:
    module = load_checker()
    required_paths = (
        Path("scripts/run_sumeragi_v2_release_gates.sh"),
        Path("ci/check_sumeragi_v2_multilane_release_inventory.sh"),
        Path("scripts/write_sumeragi_v2_release_receipt.py"),
        Path("scripts/bootstrap_sumeragi_v2_release.py"),
        Path("scripts/validate_sumeragi_v2_release_bootstrap.py"),
        Path("formal/sumeragi_v2/README.md"),
        Path("formal/sumeragi_v2/PROOF.md"),
        Path("specs/sumeragi_v2_liveness.md"),
        Path("crates/iroha_data_model/src/block/consensus_v2/finality.rs"),
        Path("integration_tests/tests/sumeragi_v2_runner.rs"),
        Path("crates/iroha_core/src/sumeragi/v2.rs"),
        Path("crates/iroha_core/src/sumeragi/v2_runner.rs"),
        Path("crates/iroha_core/src/sumeragi/v2_lane_work.rs"),
    )
    required_paths = _release_inventory_fixture_paths(module, required_paths)
    for relative in required_paths:
        destination = tmp_path / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(ROOT_DIR / relative, destination)

    assert module._production_liveness_release_inventory_errors(tmp_path) == []

    runner_path = (
        tmp_path
        / "crates"
        / "iroha_core"
        / "src"
        / "sumeragi"
        / "tests"
        / "v2_runner_unsealed_01.rs"
    )
    source = runner_path.read_text(encoding="utf-8")
    exact_retry_split = "        if calls == 2 {"
    assert source.count(exact_retry_split) == 1
    runner_path.write_text(
        source.replace(
            exact_retry_split,
            exact_retry_split.replace("if calls == 2", "if calls == 1"),
            1,
        ),
        encoding="utf-8",
    )

    errors = module._production_liveness_release_inventory_errors(tmp_path)
    assert any(
        "closed-prefix suffix-retry release regression must match exact "
        "reviewed token digest" in error
        for error in errors
    ), errors


@pytest.mark.parametrize(
    ("relative", "old", "new"),
    (
        (
            Path("formal/sumeragi_v2/README.md"),
            "current inventory to 881 tests across 43 modules.\n"
            "Together with the source-sealed command and tooling legs, the pre-network\n"
            "corridor contains 84 legs.",
            "current inventory to 881 tests across 43 modules.\n"
            "Together with the source-sealed command and tooling legs, the pre-network\n"
            "corridor contains 82 legs.",
        ),
        (
            Path("formal/sumeragi_v2/PROOF.md"),
            "current 881-test,\n43-module inventory. The complete source-sealed\n"
            "pre-network corridor\n"
            "contains 84 legs",
            "current 881-test,\n43-module inventory. The complete source-sealed\n"
            "pre-network corridor\n"
            "contains 82 legs",
        ),
        (
            Path("specs/sumeragi_v2_liveness.md"),
            "current inventory to 881\nexact tests across 43 modules and 84 pre-network legs.",
            "current inventory to 881\nexact tests across 43 modules and 82 pre-network legs.",
        ),
        (
            Path("specs/sumeragi_v2_multilane_closure_ledger.md"),
            "terminal_sweep_source_partitions_whole_units_before_any_mutation",
            "terminal_sweep_source_binds_chain_route_and_empty_post_readback",
        ),
        (
            Path("specs/sumeragi_v2_multilane_closure_ledger.md"),
            "contain exactly 531 unique required",
            "contain exactly 530 unique required",
        ),
        (
            Path("specs/sumeragi_v2_multilane_closure_ledger.md"),
            "tests: 325 core, 143 queue-journal",
            "tests: 315 core, 143 queue-journal",
        ),
        (
            Path("specs/sumeragi_v2_multilane_closure_ledger.md"),
            "exact `531/531` source consistency",
            "exact `530/531` source consistency",
        ),
    ),
    ids=(
        "readme-corridor-count",
        "proof-corridor-count",
        "liveness-corridor-count",
        "closure-ledger-terminal-test-name",
        "closure-ledger-g-unit-total",
        "closure-ledger-g-unit-core-count",
        "closure-ledger-g-unit-ratio",
    ),
)
def test_production_release_inventory_rejects_stale_liveness_corridor_claim(
    tmp_path: Path,
    relative: Path,
    old: str,
    new: str,
) -> None:
    module = load_checker()
    for fixture_relative in _release_inventory_fixture_paths(
        module,
        (
            Path("scripts/run_sumeragi_v2_release_gates.sh"),
            Path("scripts/write_sumeragi_v2_release_receipt.py"),
            Path("formal/sumeragi_v2/README.md"),
            Path("formal/sumeragi_v2/PROOF.md"),
            Path("specs/sumeragi_v2_liveness.md"),
            Path("scripts/bootstrap_sumeragi_v2_release.py"),
            Path("scripts/validate_sumeragi_v2_release_bootstrap.py"),
            Path("crates/iroha_data_model/src/block/consensus_v2/finality.rs"),
            Path("integration_tests/tests/sumeragi_v2_runner.rs"),
            Path("crates/iroha_core/src/sumeragi/v2.rs"),
            Path("crates/iroha_core/src/sumeragi/v2_runner.rs"),
        ),
    ):
        destination = tmp_path / fixture_relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(ROOT_DIR / fixture_relative, destination)

    document_path = tmp_path / relative
    source = document_path.read_text(encoding="utf-8")
    assert source.count(old) == 1
    document_path.write_text(
        source.replace(old, new, 1),
        encoding="utf-8",
    )

    errors = module._production_liveness_release_inventory_errors(tmp_path)
    assert any(
        "release inventory documentation must contain exact claim" in error
        and relative.name in error
        for error in errors
    ), errors


@pytest.mark.parametrize(
    ("relative", "old", "new", "expected_error"),
    (
        (
            Path("scripts/write_sumeragi_v2_release_receipt.py"),
            "_PRODUCTION_TEST_COUNT = 881",
            "_PRODUCTION_TEST_COUNT = 861",
            "production test count must equal the exact shell inventory count 881",
        ),
        (
            Path("scripts/write_sumeragi_v2_release_receipt.py"),
            '    "write_sumeragi_v2_release_receipt_publication.py",\n',
            "",
            "release receipt component manifest must equal",
        ),
        (
            Path("scripts/write_sumeragi_v2_release_receipt_publication.py"),
            "    return 0\n",
            "    return 0\n\n\ndef _owned_unlink_name(*_args):\n    return True\n",
            "release receipt component symbols must equal",
        ),
        (
            Path("scripts/run_sumeragi_v2_release_gates.sh"),
            "--formal-replay-principal",
            "--replay-principal",
            "terminal receipt publication must carry --formal-replay-principal exactly once",
        ),
        (
            Path("scripts/write_sumeragi_v2_release_receipt.py"),
            "bootstrap runner signed formal replay inputs are not the receipt inputs",
            "bootstrap runner formal inputs were not checked",
            "aggregate receipt must bind the signed formal replay inputs to the authenticated bootstrap environment",
        ),
        (
            Path("scripts/bootstrap_sumeragi_v2_release.py"),
            '    "formal_replay_release",\n',
            "",
            "terminal release evidence must require the signed formal replay release bundle",
        ),
        (
            Path("scripts/bootstrap_sumeragi_v2_release_receipt_replay.py"),
            'finalized["receipt"].sha256 != source_receipt.sha256\n',
            'finalized["receipt"].size != source_receipt.size\n',
            "bootstrap formal replay integration must retain the source/archive receipt equality gate exactly once",
        ),
        (
            Path("scripts/write_sumeragi_v2_release_receipt_formal_artifacts.py"),
            '"namespace": "iroha-sumeragi-v2-replay-receipt-v1",\n',
            '"namespace": "wrong-namespace",\n',
            "aggregate formal replay evidence must retain the V1 replay SSHSIG namespace exactly once",
        ),
        (
            Path("scripts/write_sumeragi_v2_release_receipt.py"),
            '("production-v2-core", "sumeragi::v2_core::tests", 38),',
            '("production-v2-core", "sumeragi::v2_core::tests", 39),',
            "production module receipt tuple must equal the exact shell",
        ),
        (
                Path("scripts/write_sumeragi_v2_release_receipt.py"),
                '        "sumeragi::authoritative_runtime_gate_tests",\n'
                "        42,\n"
                "    ),",
                '        "sumeragi::authoritative_runtime_gate_tests",\n'
                "        41,\n"
                "    ),",
            "production module receipt tuple must equal the exact shell",
        ),
        (
            Path("scripts/write_sumeragi_v2_release_receipt.py"),
            '("production-v2-adapter", "sumeragi::v2::tests", 52),',
            '("production-v2-adapter", "sumeragi::v2::tests", 49),',
            "production module receipt tuple must equal the exact shell",
        ),
        (
            Path("scripts/write_sumeragi_v2_release_receipt.py"),
                '("production-v2-effects", "sumeragi::v2_effects::tests", 66),',
                '("production-v2-effects", "sumeragi::v2_effects::tests", 65),',
            "production module receipt tuple must equal the exact shell",
        ),
        (
            Path("scripts/write_sumeragi_v2_release_receipt.py"),
            '("production-v2-runtime", "sumeragi::v2_runtime::tests", 65),',
            '("production-v2-runtime", "sumeragi::v2_runtime::tests", 64),',
            "production module receipt tuple must equal the exact shell",
        ),
        (
            Path("scripts/write_sumeragi_v2_release_receipt.py"),
            '        "production-v2-certified-serve-payload-store",\n'
            '        "sumeragi::v2_certified_serve_payload_store::tests",\n'
            "        13,\n"
            "    ),",
            '        "production-v2-certified-serve-payload-store",\n'
            '        "sumeragi::v2_certified_serve_payload_store::tests",\n'
            "        12,\n"
            "    ),",
            "production module receipt tuple must equal the exact shell",
        ),
        (
            Path("scripts/write_sumeragi_v2_release_receipt.py"),
            '("production-merge-sidecar", "merge_sidecar::tests", 118),',
            '("production-merge-sidecar", "merge_sidecar::tests", 117),',
            "production module receipt tuple must equal the exact shell",
        ),
        (
            Path("scripts/write_sumeragi_v2_release_receipt.py"),
            '("production-v2-lane-work", "sumeragi::v2_lane_work::tests", 65),',
            '("production-v2-lane-work", "sumeragi::v2_lane_work::tests", 62),',
            "production module receipt tuple must equal the exact shell",
        ),
        (
            Path("scripts/write_sumeragi_v2_release_receipt.py"),
            '        "production-v2-lifecycle-coordinator",\n'
            '        "sumeragi::v2_lifecycle_coordinator",\n'
            "        45,\n"
            "    ),",
            '        "production-v2-lifecycle-coordinator",\n'
            '        "sumeragi::v2_lifecycle_coordinator",\n'
            "        42,\n"
            "    ),",
            "production module receipt tuple must equal the exact shell",
        ),
        (
            Path("scripts/write_sumeragi_v2_release_receipt.py"),
                '        "production-v2-lifecycle-height-driver",\n'
                '        "sumeragi::v2_runner::lifecycle_height_driver::tests",\n'
                "        2,\n"
                "    ),",
                '        "production-v2-lifecycle-height-driver",\n'
                '        "sumeragi::v2_runner::lifecycle_height_driver::tests",\n'
                "        1,\n"
                "    ),",
            "production module receipt tuple must equal the exact shell",
        ),
        (
            Path("scripts/write_sumeragi_v2_release_receipt.py"),
            '("production-v2-worker", "sumeragi::v2_worker::tests", 92),',
            '("production-v2-worker", "sumeragi::v2_worker::tests", 91),',
            "production module receipt tuple must equal the exact shell",
        ),
        (
            Path("scripts/write_sumeragi_v2_release_receipt.py"),
            '("production-v2-runner", "sumeragi::v2_runner::tests", 37),',
            '("production-v2-runner", "sumeragi::v2_runner::tests", 36),',
            "production module receipt tuple must equal the exact shell",
        ),
        (
                Path("scripts/write_sumeragi_v2_release_receipt.py"),
                '        "production-irohad-network-relay",\n'
                '        "network_relay_tests",\n'
                "        5,\n"
                "    ),",
                '        "production-irohad-network-relay",\n'
                '        "network_relay_tests",\n'
                "        4,\n"
                "    ),",
            "production module receipt tuple must equal the exact shell",
        ),
        (
            Path("scripts/run_sumeragi_v2_release_gates.sh"),
            "  readonly expected_corridor_leg_count=84",
            "  readonly expected_corridor_leg_count=83",
            "sealed at 84 legs",
        ),
        (
            Path("scripts/run_sumeragi_v2_release_gates.sh"),
            '    source-sealed-workspace-tests command 0 \\\n'
            '    "${IROHA_RELEASE_CARGO_BIN} test -j1 --locked --offline --workspace" \\\n'
            "    run_cargo test --locked --offline --workspace",
            '    source-sealed-workspace-tests command 0 \\\n'
            '    "${IROHA_RELEASE_CARGO_BIN} test -j1 --locked --workspace" \\\n'
            "    run_cargo test --locked --workspace",
            "source-sealed command-success leg source-sealed-workspace-tests",
        ),
    ),
)
def test_production_release_inventory_rejects_receipt_and_command_drift(
    tmp_path: Path,
    relative: Path,
    old: str,
    new: str,
    expected_error: str,
) -> None:
    module = load_checker()
    required_paths = (
        Path("scripts/run_sumeragi_v2_release_gates.sh"),
        Path("ci/check_sumeragi_v2_multilane_release_inventory.sh"),
        Path("scripts/write_sumeragi_v2_release_receipt.py"),
        Path("formal/sumeragi_v2/README.md"),
        Path("formal/sumeragi_v2/PROOF.md"),
        Path("specs/sumeragi_v2_liveness.md"),
        Path("scripts/bootstrap_sumeragi_v2_release.py"),
        Path("scripts/validate_sumeragi_v2_release_bootstrap.py"),
        Path("crates/iroha_data_model/src/block/consensus_v2/finality.rs"),
        Path("integration_tests/tests/sumeragi_v2_runner.rs"),
        Path("crates/iroha_core/src/sumeragi/v2.rs"),
        Path("crates/iroha_core/src/sumeragi/v2_runner.rs"),
    )
    required_paths = _release_inventory_fixture_paths(module, required_paths)
    for required in required_paths:
        destination = tmp_path / required
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(ROOT_DIR / required, destination)

    path = tmp_path / relative
    source = path.read_text(encoding="utf-8")
    assert source.count(old) == 1, old
    path.write_text(source.replace(old, new, 1), encoding="utf-8")

    errors = module._production_liveness_release_inventory_errors(tmp_path)
    assert any(expected_error in error for error in errors), errors


def test_ready_validate_wal_crash_release_binding_accepts_current_source(tmp_path: Path) -> None:
    module = load_checker()
    sources = [Path(relative) for relative in module._READY_VALIDATE_WAL_CRASH_SOURCE_FILES]
    for relative in sources:
        destination = tmp_path / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(ROOT_DIR / relative, destination)
        for component in module._REVIEWED_RUST_INCLUDE_MANIFESTS.get(relative.as_posix(), ()):
            dependency = relative.parent / component
            if dependency not in sources:
                sources.append(dependency)
    assert module._ready_validate_wal_crash_replay_source_errors(tmp_path) == []


@pytest.mark.parametrize(
    ("source_index", "old", "new", "diagnostic"),
    (
        (0, "adapter.wal.append(&encoded_wal_payload)", "adapter.append_without_durable_wal(&encoded_wal_payload)", "durable append before live Sign seal"),
        (0, "frame.payload() != encoded_wal_payload.as_slice()", "false", "durable append before live Sign seal"),
        (0, "#[cfg(test)]\n    crash_after_wal_append: bool", "crash_after_wal_append: bool", "test-only bound-publication field"),
        (1, "self.file.sync_data()?", "self.file.flush()?", "actual WAL sync"),
        (2, "if let Err(source) = io.sync_data()", "if let Err(source) = io.flush()", "core WAL acknowledgment order"),
        (3, "for phase in [wire::GlobalPhase::Prepare, wire::GlobalPhase::Commit]", "for phase in [wire::GlobalPhase::Prepare]", "crash regression exact repeated replay"),
        (3, "for _ in 0..2", "for _ in 0..1", "crash regression exact repeated replay"),
        (3, 'std::fs::read(&wal_path).expect("read WAL after fresh replay and signing"),\n                durable_wal', 'std::fs::read(&wal_path).expect("read WAL after fresh replay and signing"),\n                wal_before', "crash regression exact repeated replay"),
    ),
)
def test_ready_validate_wal_crash_release_binding_rejects_mutations(
    tmp_path: Path, source_index: int, old: str, new: str, diagnostic: str,
) -> None:
    module = load_checker()
    sources = [Path(relative) for relative in module._READY_VALIDATE_WAL_CRASH_SOURCE_FILES]
    for relative in sources:
        destination = tmp_path / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(ROOT_DIR / relative, destination)
        for component in module._REVIEWED_RUST_INCLUDE_MANIFESTS.get(relative.as_posix(), ()):
            dependency = relative.parent / component
            if dependency not in sources:
                sources.append(dependency)
    baseline = module._ready_validate_wal_crash_replay_source_errors(tmp_path)
    assert baseline == [], baseline
    path = tmp_path / module._READY_VALIDATE_WAL_CRASH_SOURCE_FILES[source_index]
    source = path.read_text(encoding="utf-8")
    if source_index == 3:
        start = source.index("fn ready_validate_crash_after_wal_append_replays_exact_prepare_and_commit(")
    else:
        start = 0
    offset = source.find(old, start)
    assert offset >= 0
    path.write_text(source[:offset] + new + source[offset + len(old):], encoding="utf-8")
    errors = module._ready_validate_wal_crash_replay_source_errors(tmp_path)
    assert any(diagnostic in error for error in errors), errors


def test_release_inventory_constants_match_current_source_seal(
    tmp_path: Path,
) -> None:
    """Every release consumer binds the current production and focus seals."""

    module = load_checker()
    assert module._PRODUCTION_LIVENESS_RELEASE_COUNT == 881
    assert module._PRODUCTION_LIVENESS_RELEASE_INVENTORY_SHA256 == (
        "6045ac0993327ed787010c626227580899c1561aae9366318438840870f0c815"
    )
    assert module._PRODUCTION_LIVENESS_INVENTORY_GUARD_SHA256 == (
        "172cdeba0914a253cb22c9158b5c09fb08469ca74c55617655d5261b13bae5b7"
    )
    assert module._production_liveness_release_inventory_guard_errors(ROOT_DIR) == []
    assert module._SUMERAGI_V2_PACKAGE_LAYOUT_GUARD_SHA256 == (
        "e99da2c824b86930b76c741d2f7aa47ab16092c2f84e43550fb6362a36133268"
    )
    assert module._SUMERAGI_V2_PACKAGE_LAYOUT_VERIFIER_SHA256 == (
        "42fc1fb789e115df9f54c230ee6bfc1e1c20504a904aa20f945b6369df6d7679"
    )
    assert module._PRODUCTION_MULTILANE_FOCUS_TEST_COUNT == 531
    assert module._PRODUCTION_MULTILANE_G_UNIT_TSV_LINE_COUNT == 532
    assert module._PRODUCTION_MULTILANE_FOCUS_INVENTORY_SHA256 == (
        "d56dd7d418492418aaaec6f1626bcf7f6d6aca3388f7526d76b3aac49766fd81"
    )
    assert module._PRODUCTION_LIFECYCLE_INGRESS_PUBLICATION_FENCE_ITEM_SHA256 == {
        "PreparedFairIngressQueueWitness::lock_exact_dequeue_retaining": (
            "66d33b07c062bd6dc4a1b879b0b3624bc0403e59305cbc44763d409f97d109fc"
        ),
        "LockedPreparedFairIngressExactDequeue::commit": (
            "abdd5434d703b75f26bb2053ac05942564deffb181ddfe040609f6583405ebe9"
        ),
        "locked_publication_fence_serializes_same_wire_and_reenqueues_after_commit": (
            "ea093accfdb33740bc7f21e9c26b17e74a1d7600c885ff45a5718caed8cb457a"
        ),
        "locked_publication_fence_serializes_unrelated_append_and_preserves_it": (
            "c88fcd11bd701f1a67ffc441fe1cc4bdc08f9be32e5a8270373ea83335a6131f"
        ),
        "dropping_locked_publication_fence_releases_producer_without_dequeue": (
            "a31983eba320245b25089ebfcbc6fbd5a5c024fc76b81946329510cf9177e687"
        ),
    }
    assert module._PRODUCTION_READY_PROPOSAL_SIGN_PREEMPTION_ITEM_SHA256 == {
        "scheduler::ProductionLifecycleOwnerV1::ready_proposal_sign_preempts_bounded_producer_point": (
            "98286d3d592024081c92afae2353f604ecbebf804c749c13e0c9daa26b17c016"
        ),
        "height::LifecycleReadyProposalSignPreemptionPermitV1": (
            "6a1f9f015e100d2c21a2059b2b5ed299c58d4084375d80117f2ff2d9baf565e6"
        ),
        "height::LifecycleProducerClaimDispositionV1::ready_proposal_sign_preemption_permit": (
            "9700af71a07b9b6e8c935f44e6e447c3d2087f89508733c6f124a3d4beedce51"
        ),
        "height::drain_lifecycle_v2_ingress": (
            "bbd77022da85d8d4ae7a7b1114483f3d3437e8fdbce14de7cb702b5716f26ddd"
        ),
        "height_test::only_an_eligible_claim_can_preempt_an_ordinary_head_for_ready_proposal_sign": (
            "dd96ca9fb8271e423099f6a019259cbfa524d73d86f07d1afdd377aa80dc8e76"
        ),
        "driver::LaunchedProductionLifecycleV1::drive_completion_pre_gate_with_ready_proposal_sign_preemption": (
            "0fabc0723a3288b463bf55b2cc7a02638cb1627967ba717b169520b7bee3eaf2"
        ),
        "driver::LaunchedProductionLifecycleV1::drive_completion_pre_gate_inner": (
            "f10f0a6f3b6824d8f264dc9bf30538ad1563d8121c75c14d26d37bbea30c5cb4"
        ),
        "driver::ActivatedProductionLifecycleV1::drive_completion_pre_gate_with_ready_proposal_sign_preemption": (
            "bb485fa1d93cd1748cd6d8f0c7152c4afb78b7bdcbdb5423aa22eb2c55129b77"
        ),
        "worker_test::LifecyclePlannerIoFixture::publish_auxiliary_completion_fixture": (
            "c3b5921a9f581e7ad7bbb44e93ad42de8ec1fa6eb8bd62aaa49cdbefa70327c4"
        ),
        "launch_test::LaunchedProductionLifecycleV1::install_ordinary_completion_head_for_ready_sign_test": (
            "f9f33cf99e1c38a2ebaea00d376fe5bfff7d434620dc46ce260665b7e45c2f8f"
        ),
        "launch_test::LaunchedProductionLifecycleV1::ordinary_completion_head_retained_for_ready_sign_test": (
            "a5bba2319108935316c46e2402dcfce41c9e2e0ba87107dda239344b7cefe028"
        ),
        "launch_test::LaunchedProductionLifecycleV1::drain_ordinary_completion_head_for_ready_sign_test": (
            "edeec434ad30fb28ddc4cb526096bafa6d6f8bd8e8df92995e5350c0f95111db"
        ),
        "dispatch_test::local_proposal_intent_live_wal_sign_fixture": (
            "14ec208611139775c959d7cc44d718925d179c7d822c1cb92bea3018f6215489"
        ),
        "wal_test::ready_proposal_sign_boundary_predicate_authenticates_exact_control_carrier": (
            "c4b40eb74bfcafcbe413991d85044ad6c857d1faf1a5294944705449a13f269b"
        ),
        "wal_test::ready_local_proposal_sign_and_exact_output_precede_pending_timeout_certificate": (
            "97b485f11895e0f3e0273d978498d16caa02850e8bf4d9fee8e7434069865b13"
        ),
    }
    assert (
        "_production_liveness_release_inventory_guard_errors"
        in module._production_liveness_release_inventory_errors.__code__.co_names
    )
    assert module._sumeragi_v2_package_layout_guard_errors(ROOT_DIR) == []

    package_root = tmp_path / "package-layout"
    package_guard = package_root / "scripts" / "check_sumeragi_v2_package_layout.sh"
    package_verifier = package_root / "scripts" / "verify_sumeragi_v2.sh"
    package_core_root = (
        package_root / "crates" / "iroha_core" / "src" / "sumeragi"
    )
    package_guard.parent.mkdir(parents=True)
    package_core_root.mkdir(parents=True)
    shutil.copy2(
        ROOT_DIR / "scripts" / "check_sumeragi_v2_package_layout.sh",
        package_guard,
    )
    shutil.copy2(ROOT_DIR / "scripts" / "verify_sumeragi_v2.sh", package_verifier)
    shutil.copy2(
        ROOT_DIR / "crates" / "iroha_core" / "src" / "sumeragi" / "v2_core.rs",
        package_core_root / "v2_core.rs",
    )
    shutil.copytree(
        ROOT_DIR
        / "crates"
        / "iroha_core"
        / "src"
        / "sumeragi"
        / "v2_core",
        package_core_root / "v2_core",
    )
    bash = shutil.which("bash")
    assert bash is not None
    baseline = subprocess.run(
        [bash, str(package_guard)],
        check=False,
        capture_output=True,
        text=True,
        timeout=30,
    )
    assert baseline.returncode == 0, baseline.stderr

    refinement = package_core_root / "v2_core" / "refinement.rs"
    refinement_source = refinement.read_text(encoding="utf-8")
    layout_mutations = (
        (
            refinement_source + '\n#[path = "shadow.rs"]\nmod shadow;\n',
            "second path attribute",
        ),
        (
            refinement_source.replace(
                '#[path = "refinement_cases.rs"]',
                '#[path = "../refinement_cases.rs"]',
                1,
            ),
            "parent-relative path attribute",
        ),
        (
            refinement_source.replace(
                '#[cfg(test)]\n#[path = "refinement_cases.rs"]',
                '#[path = "refinement_cases.rs"]',
                1,
            ),
            "non-test path attribute",
        ),
    )
    for mutation, description in layout_mutations:
        refinement.write_text(mutation, encoding="utf-8")
        result = subprocess.run(
            [bash, str(package_guard)],
            check=False,
            capture_output=True,
            text=True,
            timeout=30,
        )
        assert result.returncode != 0, description
        assert (
            "only the reviewed package-local refinement test split and "
            "identity-preserving nested include"
            in result.stderr
        )
    refinement.write_text(refinement_source, encoding="utf-8")

    refinement_cases = package_core_root / "v2_core" / "refinement_cases.rs"
    refinement_cases_source = refinement_cases.read_text(encoding="utf-8")
    nested_include = 'include!("refinement_cases/terminal_body_pipeline.rs");'
    assert refinement_cases_source.count(nested_include) == 1
    refinement_cases.write_text(
        refinement_cases_source.replace(
            nested_include,
            '#[path = "refinement_cases/terminal_body_pipeline.rs"]\n'
            "mod terminal_body_pipeline;",
            1,
        ),
        encoding="utf-8",
    )
    nested_result = subprocess.run(
        [bash, str(package_guard)],
        check=False,
        capture_output=True,
        text=True,
        timeout=30,
    )
    assert nested_result.returncode != 0, "nested module identity drift"
    assert (
        "only the reviewed package-local refinement test split and "
        "identity-preserving nested include"
        in nested_result.stderr
    )
    refinement_cases.write_text(refinement_cases_source, encoding="utf-8")

    package_guard_source = package_guard.read_text(encoding="utf-8")
    package_guard.write_text(
        package_guard_source.replace("set -euo pipefail", "set +e", 1),
        encoding="utf-8",
    )
    errors = module._sumeragi_v2_package_layout_guard_errors(package_root)
    assert any(
        "package-layout guard source SHA-256 must equal" in error
        for error in errors
    ), errors
    package_guard.write_text(package_guard_source, encoding="utf-8")

    invocation = 'bash "$REPO_ROOT/scripts/check_sumeragi_v2_package_layout.sh"'
    verifier_source = package_verifier.read_text(encoding="utf-8")
    assert verifier_source.splitlines().count(invocation) == 1
    package_verifier.write_text(
        verifier_source.replace(invocation, "true # skipped package-layout guard", 1),
        encoding="utf-8",
    )
    errors = module._sumeragi_v2_package_layout_guard_errors(package_root)
    assert any(
        "must invoke the package-layout guard exactly once" in error
        for error in errors
    ), errors

    checker_source = SCRIPT.read_text(encoding="utf-8")
    validate_body = checker_source.split("def validate_ledger(", 1)[1].split(
        "\ndef ",
        1,
    )[0]
    assert (
        validate_body.count(
            "errors.extend(_sumeragi_v2_package_layout_guard_errors(ROOT_DIR))"
        )
        == 1
    )

    receipt_spec = importlib.util.spec_from_file_location(
        "sumeragi_v2_release_receipt_current_inventory",
        ROOT_DIR / "scripts" / "write_sumeragi_v2_release_receipt.py",
    )
    assert receipt_spec is not None
    assert receipt_spec.loader is not None
    receipt_module = importlib.util.module_from_spec(receipt_spec)
    sys.modules[receipt_spec.name] = receipt_module
    receipt_spec.loader.exec_module(receipt_module)
    assert receipt_module._PRODUCTION_TEST_COUNT == 881
    assert receipt_module._G_UNIT_TEST_COUNT == 531
    assert sum(count for _, _, count in receipt_module._PRODUCTION_MODULES) == 881
    receipt_module_counts = {
        module_name: count
        for _leg_id, module_name, count in receipt_module._PRODUCTION_MODULES
    }
    assert receipt_module_counts["kura::tests"] == 18
    assert receipt_module_counts["sumeragi::authoritative_runtime_gate_tests"] == 42
    assert receipt_module_counts["queue::tests"] == 1
    assert receipt_module_counts["native_amx::participant_application_role_tests"] == 6
    assert receipt_module_counts["sumeragi::v2::tests"] == 52
    assert receipt_module_counts["sumeragi::v2_effects::tests"] == 66
    assert receipt_module_counts["sumeragi::v2_lane_work::tests"] == 65
    assert receipt_module_counts["sumeragi::v2_runtime::tests"] == 65
    assert receipt_module_counts["sumeragi::v2_certified_serve_payload_store::tests"] == 13
    assert receipt_module_counts["sumeragi::v2_lifecycle_coordinator"] == 45
    assert receipt_module_counts["sumeragi::v2_runner::tests"] == 37
    assert receipt_module_counts["network::tests"] == 84
    assert receipt_module_counts["sumeragi::v2_runner::lifecycle_height_driver::tests"] == 2
    assert receipt_module_counts["sumeragi::v2_worker::tests"] == 92
    assert receipt_module_counts["block::consensus_v2::tests"] == 3
    assert "sumeragi::v2_core::network_simulation" not in receipt_module_counts
    assert (
        sum(count for _, _, _, count, _ in receipt_module._G_UNIT_GROUPS)
        == 531
    )


def test_proof_ledger_tests_have_unique_reviewed_component_providers() -> None:
    """Reject lexical test shadows and ownership drift across case components."""
    expected_component_providers = {
        "test_proof_ledger_tests_have_unique_reviewed_component_providers":
            "sumeragi_v2_proof_ledger_release_inventory_cases.py",
        "test_release_inventory_checker_has_one_component_owned_provider":
            "sumeragi_v2_proof_ledger_release_inventory_cases.py",
        "test_release_corridor_rejects_network_skips_and_zero_test_filters":
            "sumeragi_v2_proof_ledger_corridor_acceptance_cases.py",
        "test_release_inventory_constants_match_current_source_seal":
            "sumeragi_v2_proof_ledger_release_inventory_cases.py",
        "_replace_late_lane_recovery_tokens":
            "sumeragi_v2_proof_ledger_release_inventory_cases.py",
        "_late_lane_recovery_runtime_mutations":
            "sumeragi_v2_proof_ledger_release_inventory_cases.py",
        "test_late_lane_recovery_contract_mutations_authenticate_actual_owner":
            "sumeragi_v2_proof_ledger_release_inventory_cases.py",
        "_release_corridor_production_inventory":
            "sumeragi_v2_proof_ledger_release_inventory_cases.py",
        "test_release_corridor_inventory_parser_preserves_all_owners":
            "sumeragi_v2_proof_ledger_release_inventory_cases.py",
        "test_async_source_fidelity_pins_validator_progress_capacity":
            "sumeragi_v2_proof_ledger_async_source_cases.py",
        "test_ownership_n1_pins_exact_ingress_and_deferred_progress_geometry":
            "sumeragi_v2_proof_ledger_async_source_cases.py",
        "test_leader_wire_physical_ingress_rejects_semantic_mutations":
            "sumeragi_v2_proof_ledger_async_source_cases.py",
        "test_local_runner_service_contract_rejects_production_loop_mutations":
            "sumeragi_v2_proof_ledger_async_fairness_cases.py",
        "test_async_source_fidelity_rejects_reviewed_theorem_omission":
            "sumeragi_v2_proof_ledger_async_fairness_cases.py",
        "test_exact_output_production_source_mutations_fail_closed":
            "sumeragi_v2_proof_ledger_exact_output_cases.py",
        "test_temporal_proof_promotions_require_prerequisites_and_ledger_order":
            "sumeragi_v2_proof_ledger_trace_dependency_cases.py",
        "test_successor_run_inner_parser_rejects_neighbor_lookalike":
            "sumeragi_v2_proof_ledger_successor_production_cases.py",
        "test_successor_production_source_mapping_mutations_fail_closed":
            "sumeragi_v2_proof_ledger_successor_production_cases.py",
        "complete_ledger": "sumeragi_v2_proof_ledger_release_inventory_cases.py",
        "write_tlaps_fixture_logs":
            "sumeragi_v2_proof_ledger_release_inventory_cases.py",
        "build_test_evidence":
            "sumeragi_v2_proof_ledger_release_inventory_cases.py",
        "complete_cross_tool_ledger":
            "sumeragi_v2_proof_ledger_release_inventory_cases.py",
        "build_cross_tool_fixture":
            "sumeragi_v2_proof_ledger_release_inventory_cases.py",
    }
    def provider_errors(sources: tuple[tuple[Path, str], ...]) -> list[str]:
        providers: dict[str, list[str]] = {}
        for path, source in sources:
            tree = ast.parse(source, filename=str(path))
            for node in tree.body:
                if not isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef)):
                    continue
                if (
                    not node.name.startswith("test_")
                    and node.name not in expected_component_providers
                ):
                    continue
                providers.setdefault(node.name, []).append(path.name)
        errors = [
            f"test provider {name} is not unique: {locations!r}"
            for name, locations in sorted(providers.items())
            if len(locations) != 1
        ]
        for name, expected_provider in expected_component_providers.items():
            if providers.get(name) != [expected_provider]:
                errors.append(
                    f"test provider {name} must be owned by "
                    f"{expected_provider}; found {providers.get(name)!r}"
                )
        return errors
    main_path = Path(__file__)
    canonical_sources = (
        (main_path, main_path.read_text(encoding="utf-8")),
        *(
            (path, path.read_text(encoding="utf-8"))
            for path in (
                main_path.with_name(filename)
                for filename in PROOF_LEDGER_TEST_COMPONENT_FILES
            )
        ),
    )
    assert provider_errors(canonical_sources) == []
    target = "test_exact_output_production_source_mutations_fail_closed"
    shadow = f"\n\ndef {target}():\n    pass\n"
    mutated_sources = (
        (canonical_sources[0][0], canonical_sources[0][1] + shadow),
        *canonical_sources[1:],
    )
    errors = provider_errors(mutated_sources)
    assert any(
        error.startswith(f"test provider {target} is not unique:")
        for error in errors
    ), errors


def test_release_inventory_checker_has_one_component_owned_provider() -> None:
    """Reject monolithic shadows of component-owned checker providers."""
    expected_providers = {
        "_production_liveness_release_inventory_errors": (
            "sumeragi_v2_proof_ledger_release_inventory_contracts.py"
        ),
        "_cross_tool_kernel_views": (
            "sumeragi_v2_proof_ledger_cross_tool_contracts.py"
        ),
    }
    def provider_errors(sources: tuple[tuple[Path, str], ...]) -> list[str]:
        providers: dict[str, list[str]] = {}
        for path, source in sources:
            for node in ast.parse(source, filename=str(path)).body:
                if not isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef)):
                    continue
                if node.name in expected_providers:
                    providers.setdefault(node.name, []).append(path.name)
        return [
            f"checker provider {name} must be uniquely component-owned; "
            f"found {providers.get(name)!r}"
            for name, expected_provider in expected_providers.items()
            if providers.get(name) != [expected_provider]
        ]
    canonical_sources = tuple(
        (path, path.read_text(encoding="utf-8"))
        for path in checker_source_paths()
    )
    assert provider_errors(canonical_sources) == []
    shadows = {
        "_production_liveness_release_inventory_errors": (
            "\n\ndef _production_liveness_release_inventory_errors(repo_root=ROOT_DIR):\n"
            "    return []\n"
        ),
        "_cross_tool_kernel_views": (
            "\n\ndef _cross_tool_kernel_views(claim):\n"
            "    return ()\n"
        ),
    }
    for name, shadow in shadows.items():
        mutated_sources = tuple(
            (path, source + shadow if path == SCRIPT else source)
            for path, source in canonical_sources
        )
        errors = provider_errors(mutated_sources)
        assert len(errors) == 1
        assert name in errors[0] and SCRIPT.name in errors[0], errors
