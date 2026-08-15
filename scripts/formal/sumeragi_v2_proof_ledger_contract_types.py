# Executed lexically in check_sumeragi_v2_proof_ledger.py; do not import directly.

@dataclass(frozen=True)
class CrossToolProductionCallContract:
    """One authoritative production invocation of a verified pure kernel."""

    source: str
    item: str
    projection: str
    required_expression: str
    brace_context: tuple[tuple[str, ...], ...] = ()
    item_token_sha256: str | None = None
    unfrozen_reason: str | None = None
    gate_call_count: int = 1
    gate_arguments: tuple[str, ...] = ()
    token_consumptions: tuple[str, ...] = ()
    mutation_boundaries: tuple[str, ...] = ()
    mutation_authorization_indices: tuple[int, ...] = ()
    projection_bindings: tuple[str, ...] = ()


@dataclass(frozen=True)
class CrossToolLinkedConsumerContract:
    """A mutation consumer authorized by a checked helper's successful return."""

    source: str
    item: str
    required_expression: str
    mutation_boundaries: tuple[str, ...]
    brace_context: tuple[tuple[str, ...], ...] = ()
    item_token_sha256: str | None = None
    token_consumptions: tuple[str, ...] = ()


@dataclass(frozen=True)
class CrossToolTotalGateContract:
    """One exact total production/Verus gate for a reviewed pure kernel."""

    name: str
    parameters: str
    production_return: str
    verus_return: str
    kernel_arguments: str
    theorem_arguments: str
    success_value: str
    production_item_sha256: str
    verus_item_sha256: str
    production_visibility: str = "pub(crate)"
    verus_kernel_arguments: str | None = None
    production_success_value: str | None = None


@dataclass(frozen=True)
class CrossToolSourceItemSeal:
    """One code-owned token seal in a cross-tool claim's identity closure."""

    source: str
    item: str
    item_token_sha256: str
    kind: str = "item"
    brace_context: tuple[tuple[str, ...], ...] = ()
    required_expressions: tuple[str, ...] = ()


@dataclass(frozen=True)
class CrossToolProjectionBuilderContract:
    """One exact Verus projection builder used by a reviewed kernel call."""

    name: str
    parameters: str
    return_type: str
    item_token_sha256: str


@dataclass(frozen=True)
class CrossToolSupplementalKernelContract:
    """An additional production/Verus kernel required by one theorem."""

    verified_kernel: str
    verified_kernel_source: str
    verified_kernel_parameters: str
    verified_kernel_body: str
    theorem_kernel_projection: str
    theorem_projection_builders: tuple[CrossToolProjectionBuilderContract, ...]
    verified_kernel_const: bool = True
    verified_kernel_public: bool = False
    verified_kernel_shared_macro_sha256: tuple[tuple[str, str], ...] = ()
    production_call_sites: tuple[CrossToolProductionCallContract, ...] = ()
    total_gate: CrossToolTotalGateContract | None = None
    auxiliary_verus_theorem: str | None = None
    auxiliary_verus_parameters: str | None = None
    auxiliary_verus_theorem_item_sha256: str | None = None


@dataclass(frozen=True)
class CrossToolClaimContract:
    """One immutable Rust/Verus-to-TLA production refinement claim."""

    constant: str
    verus_theorem: str
    verus_source: str
    production_sources: tuple[str, ...]
    proof_mode: str = "legacy_requires_builder"
    # The exact proof/kernel/call-site contract is intentionally optional while
    # the corresponding ledger entry is specified_unproved.  Promotion is
    # fail-closed until every field is supplied and source validation below
    # proves the exact normalized shape.  This lets the ledger describe future
    # work without accepting placeholder proofs such as `ensures true`.
    verus_parameters: str | None = None
    verus_requires: str | None = None
    verus_ensures: str | None = None
    verus_theorem_item_sha256: str | None = None
    verified_kernel: str | None = None
    verified_kernel_source: str | None = None
    verified_kernel_parameters: str | None = None
    verified_kernel_body: str | None = None
    verified_kernel_const: bool = True
    verified_kernel_public: bool = False
    verified_kernel_shared_macro_sha256: tuple[tuple[str, str], ...] = ()
    theorem_kernel_projection: str | None = None
    theorem_projection_builder: str | None = None
    theorem_projection_builder_parameters: str | None = None
    theorem_projection_builder_return: str | None = None
    theorem_projection_builder_item_sha256: str | None = None
    source_item_seals: tuple[CrossToolSourceItemSeal, ...] = ()
    production_call_sites: tuple[CrossToolProductionCallContract, ...] = ()
    supplemental_kernels: tuple[CrossToolSupplementalKernelContract, ...] = ()
    total_gate: CrossToolTotalGateContract | None = None
    linked_consumers: tuple[CrossToolLinkedConsumerContract, ...] = ()


@dataclass(frozen=True)
class CrossToolObligationContract:
    """Canonical cross-tool discharge contract for one ledger obligation."""

    obligation_id: str
    module: str
    ledger_symbol: str
    tla_theorem: str
    tla_statement: str
    claims: tuple[CrossToolClaimContract, ...]
    ledger_declaration_kind: str | None = None
    ledger_statement: str | None = None
    tla_proof: str | None = None


@dataclass(frozen=True)
class PromotionProofTargetContract:
    """One exact theorem whose strict range run can support promotion."""

    obligation_id: str
    kind: str
    ledger_module: str
    provider_module: str
    theorem: str
    # The first non-promotional pass records only a positive count.  A reviewer
    # may freeze the observed count here after that pass; evidence generation
    # never edits this code-owned contract.
    expected_obligations: int | None = None
