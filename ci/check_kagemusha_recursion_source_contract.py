#!/usr/bin/env python3
"""Static Kagemusha recursion/source-residency contracts.

These checks intentionally inspect implementation source.  Keeping them here
avoids compiling almost nine hundred lines of Rust substring tests while the
authenticated readiness gate retains the same structural coverage.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

PROVIDER = "ci/check_kagemusha_recursion_source_contract.py"
ADAPTER = "crates/iroha_core/src/zk/kagemusha_recursion_adapter.rs"
GENERATED = (
    "crates/iroha_core/src/zk/kagemusha_recursion_adapter/generated_artifacts.rs"
)
PROBE = "crates/iroha_core/src/zk/kagemusha_recursion_adapter/k17_probe.rs"
BUILDER = (
    "crates/iroha_core/src/zk/kagemusha_recursion_adapter/"
    "serialized_audit_builder_v7.rs"
)
RELEASE = (
    "crates/iroha_core/src/zk/kagemusha_recursion_adapter/"
    "serialized_audit_release_proof_v7.rs"
)
BRIDGE = (
    "crates/iroha_core/src/zk/kagemusha_recursion_adapter/"
    "serialized_audit_bridge_v7.rs"
)
VECTOR = (
    "crates/iroha_core/src/zk/kagemusha_recursion_adapter/"
    "serialized_audit_vector_v7.rs"
)
SERIALIZED = "crates/iroha_core/src/zk/kagemusha_serialized_audit_v7.rs"
BENCHMARK = "crates/iroha_core/src/bin/kagemusha_recursive_spend_v4_memory_benchmark.rs"
BENCHMARK_WRAPPER = "scripts/run_kagemusha_v4_generation_benchmark.py"
FACADE = "crates/iroha_core/src/zk/kagemusha_v2.rs"
BUNDLE = "crates/iroha_core/src/bin/kagemusha_recursive_spend_v4_bundle.rs"
CATALOG = (
    "crates/iroha_core/src/smartcontracts/isi/offline/"
    "kagemusha_terminal_registry_v4.rs"
)
KAGAMI = "crates/iroha_kagami/src/kagemusha.rs"

_IN_GATE = globals().get("_KAGEMUSHA_RECURSION_SOURCE_CONTRACT_CONTEXT_V1") is True
_SOURCE = globals().get("_KAGEMUSHA_RECURSION_SOURCE_CONTRACT_SOURCE_V1")
if _IN_GATE and (not isinstance(_SOURCE, str) or not _SOURCE):
    raise RuntimeError("recursion source-contract provider requires its exact loaded bytes")
if not _IN_GATE:
    _SOURCE = Path(__file__).read_text(encoding="utf-8")


def _read(
    root: Path, relative: str, overrides: dict[str, str], errors: list[str]
) -> str:
    if relative in overrides:
        return overrides[relative]
    try:
        return (root / relative).read_text(encoding="utf-8")
    except (OSError, UnicodeError) as error:
        errors.append(f"{relative}: could not read source contract input: {error}")
        return ""


def _section(
    text: str,
    relative: str,
    errors: list[str],
    start: str,
    end: str | None = None,
) -> str:
    offset = text.find(start)
    if offset < 0:
        errors.append(f"{relative}: missing source-contract boundary {start!r}")
        return ""
    result = text[offset + len(start) :]
    if end is not None:
        finish = result.find(end)
        if finish < 0:
            errors.append(f"{relative}: missing source-contract boundary {end!r}")
            return ""
        result = result[:finish]
    return result


def _require(text: str, relative: str, errors: list[str], *needles: str) -> None:
    for needle in needles:
        if needle not in text:
            errors.append(f"{relative}: missing source contract {needle!r}")


def _forbid(text: str, relative: str, errors: list[str], *needles: str) -> None:
    for needle in needles:
        if needle in text:
            errors.append(f"{relative}: forbidden source contract remains {needle!r}")


def _count(
    text: str, relative: str, errors: list[str], needle: str, expected: int
) -> None:
    observed = text.count(needle)
    if observed != expected:
        errors.append(
            f"{relative}: {needle!r} count is {observed}, expected {expected}"
        )


def _ordered(
    text: str, relative: str, errors: list[str], *needles: str, reverse_last: bool = False
) -> None:
    offsets: list[int] = []
    for index, needle in enumerate(needles):
        offset = text.rfind(needle) if reverse_last and index == len(needles) - 1 else text.find(needle)
        if offset < 0:
            errors.append(f"{relative}: missing ordered source contract {needle!r}")
            return
        offsets.append(offset)
    if offsets != sorted(offsets) or len(set(offsets)) != len(offsets):
        errors.append(f"{relative}: source contracts are not ordered: {needles!r}")


def _adapter_contracts(text: str, errors: list[str]) -> None:
    header = _section(
        text,
        ADAPTER,
        errors,
        "fn constrain_kagemusha_compact_eq_header_v5(",
        "fn constrain_kagemusha_output_frontier_v4",
    )
    _require(
        header,
        ADAPTER,
        errors,
        "layout: &KagemushaPastaPublicLayoutV4",
        "usize::try_from(layout.instance_column_limbs)",
        "compact.len() != expected_compact_len",
        "let operation_step =",
        "ctx.constrain_equal(",
        "range.range_check(",
    )
    _forbid(header, ADAPTER, errors, "compact.len() != 64")
    signature = header.split(") -> Result<(), String>", 1)[0]
    _forbid(signature, ADAPTER, errors, "proof_step_count")
    _count(
        header,
        ADAPTER,
        errors,
        "KAGEMUSHA_COMPACT_PROOF_STEP_COUNT_OFFSET_V5",
        2,
    )
    _forbid(header, ADAPTER, errors, "Fp::from(u64::from(proof_step_count))")

    generator = _section(
        text,
        ADAPTER,
        errors,
        "fn generate_kagemusha_pasta_cycle_artifacts_in_pool_v5(",
        "/// Produce and immediately self-verify one concrete V4 StepEq proof.",
    )
    _require(
        generator,
        ADAPTER,
        errors,
        "let compile_config = || kagemusha_ipa_compile_config_v4(public_len);",
        "keygen_vk_consuming_with(",
        "keygen_pk_consuming_with(",
        "drop(step_eq_verifying_key)",
        "drop(step_ep_verifying_key)",
    )
    _count(generator, ADAPTER, errors, "compile_config()", 2)
    _forbid(
        generator,
        ADAPTER,
        errors,
        "Config::ipa().with_num_instance",
        "build_kagemusha_step_circuits_v4(",
        "build_kagemusha_step_circuits_with_mode_v4(",
        "drop(step_eq_live_keygen_circuit)",
        "drop(step_ep_live_keygen_circuit)",
    )
    _ordered(
        generator,
        ADAPTER,
        errors,
        "let step_eq_seed = kagemusha_eq_bootstrap_seed_v4",
        "let step_eq_parameter_spool = kagemusha_eq_parameters_bytes_v4",
        "drop(step_eq_params);",
        "let step_ep_params = ParamsIPA::<EpAffine>::new",
        "let step_ep_seed = kagemusha_ep_bootstrap_seed_v4",
        "let step_eq_params = parse_kagemusha_params_v4::<EqAffine>",
    )
    _ordered(
        generator,
        ADAPTER,
        errors,
        "failed to stream Kagemusha V5 Eq processed proving key",
        "let step_ep_live_circuit = build_kagemusha_step_ep_circuit_v5",
    )

    fields = _section(
        text,
        ADAPTER,
        errors,
        "pub(crate) struct KagemushaPastaCycleProverV4 {",
        "impl std::ops::Deref for KagemushaPastaCycleProverV4",
    )
    _require(
        fields,
        ADAPTER,
        errors,
        "step_eq_verifying_key_bytes: Vec<u8>",
        "step_ep_verifying_key_bytes: Vec<u8>",
    )
    _forbid(fields, ADAPTER, errors, "step_eq_verifying_key:", "step_ep_verifying_key:")
    prove = _section(
        text, ADAPTER, errors, "    fn prove_step_v4(", "/// Circuit-side parent-proof"
    )
    _forbid(
        prove,
        ADAPTER,
        errors,
        "step_eq_proving_key.get_vk()",
        "step_ep_proving_key.get_vk()",
    )
    _ordered(
        prove,
        ADAPTER,
        errors,
        "let step_eq_proving_key =",
        "let (step_eq_proof_bytes, step_eq_verifying_key) = prove_step_eq_v4",
        "drop(step_eq_verifying_key)",
        "let step_ep = build_kagemusha_step_ep_circuit_v5",
        "drop(step_ep_verifying_key)",
        "let step_eq_terminal_verifying_key =",
    )

    for start, end in (
        (
            "fn qualify_kagemusha_eq_artifacts_v4(",
            "fn qualify_kagemusha_ep_artifacts_v4(",
        ),
        (
            "fn qualify_kagemusha_ep_artifacts_v4(",
            "pub(super) fn qualify_kagemusha_authenticated_artifact_source_v4(",
        ),
    ):
        qualification = _section(text, ADAPTER, errors, start, end)
        _require(qualification, ADAPTER, errors, "preflight_kagemusha_pk_from_source_v4(")
        _forbid(
            qualification,
            ADAPTER,
            errors,
            "load_kagemusha_eq_proving_key_from_source_v4(",
            "load_kagemusha_ep_proving_key_from_source_v4(",
            "ProvingKey::read",
        )
    eq_runtime = _section(
        text,
        ADAPTER,
        errors,
        "fn load_kagemusha_source_eq_prover_material_v4(",
        "fn load_kagemusha_source_ep_prover_material_v4(",
    )
    ep_runtime = _section(
        text,
        ADAPTER,
        errors,
        "fn load_kagemusha_source_ep_prover_material_v4(",
        "fn load_kagemusha_source_eq_recursion_material_v4(",
    )
    _require(
        eq_runtime,
        ADAPTER,
        errors,
        "load_kagemusha_eq_proving_key_from_qualified_source_v4",
    )
    _require(
        ep_runtime,
        ADAPTER,
        errors,
        "load_kagemusha_ep_proving_key_from_qualified_source_v4",
    )
    for start, end in (
        ("fn parse_kagemusha_eq_pk_spool_v5(", "fn parse_kagemusha_ep_pk_spool_v5("),
        ("fn parse_kagemusha_ep_pk_spool_v5(", "pub(crate) struct KagemushaPastaCycleProverV4"),
    ):
        parser = _section(text, ADAPTER, errors, start, end)
        _require(parser, ADAPTER, errors, "catch_unwind", "proving-key reader panicked")

    verifier = _section(
        text,
        ADAPTER,
        errors,
        "pub fn verify_candidate_recursive_step_two_receipt_v4<F>(",
        "struct KagemushaEqBootstrapSeedV4",
    )
    _count(verifier, ADAPTER, errors, "authenticate_kagemusha_receipt_pk_spool_v5(", 2)
    _count(
        verifier,
        ADAPTER,
        errors,
        "KagemushaPastaCycleTerminalVerifierV4::from_validated_artifact_loader(",
        1,
    )
    _forbid(
        verifier,
        ADAPTER,
        errors,
        "from_candidate_artifact_spool_loader(",
        "parse_kagemusha_eq_pk_spool_v5(",
        "parse_kagemusha_ep_pk_spool_v5(",
        "ProvingKey::read",
    )

    prepass = _section(
        text,
        ADAPTER,
        errors,
        "fn collect_kagemusha_scalar_audits_v4<C>(",
        "fn scalar_field_parent_count_v4",
    )
    _require(prepass, ADAPTER, errors, "BaseCircuitBuilder::<C::ScalarExt>::new(true)")
    _forbid(prepass, ADAPTER, errors, "BaseCircuitBuilder::<C::ScalarExt>::new(false)")
    for start, end in (
        (
            "impl halo2_proofs::plonk::Circuit<Fp> for KagemushaStepEqCircuitV4",
            "/// Production StepEp circuit type",
        ),
        (
            "impl halo2_proofs::plonk::Circuit<Fq> for KagemushaStepEpCircuitV4",
            "#[derive(Clone, Copy, Debug, PartialEq, Eq)]",
        ),
    ):
        implementation = _section(text, ADAPTER, errors, start, end)
        _require(
            implementation,
            ADAPTER,
            errors,
            "type FloorPlanner = halo2_proofs::circuit::V1",
            "fn synthesize_for_measurement(",
            "self.builder.reset_synthesis_state()",
            "halo2_proofs::release_allocator_slack()",
        )
        _forbid(implementation, ADAPTER, errors, "SimpleFloorPlanner")

    wrapper = _section(
        text,
        ADAPTER,
        errors,
        "pub fn generate_kagemusha_pasta_cycle_artifacts_v4(",
        "fn generate_kagemusha_pasta_cycle_artifacts_in_pool_v5(",
    )
    _require(
        wrapper,
        ADAPTER,
        errors,
        ".num_threads(KAGEMUSHA_GENERATION_RAYON_THREADS_V5)",
        "pool.install(move ||",
    )
    profile = _section(
        text,
        ADAPTER,
        errors,
        "fn validate_kagemusha_profile_protocol_v4<C>(",
        "fn terminal_validate_kagemusha_eq_bootstrap_v4(",
    )
    _forbid(
        profile,
        ADAPTER,
        errors,
        "keygen_vk",
        "kagemusha_bootstrap_verifying_key_v1",
        "validate_bootstrap_protocol",
    )
    _require(
        profile,
        ADAPTER,
        errors,
        "kagemusha_compiled_protocol_structure_sha256",
        "KagemushaStepBootstrapV4::decode_authenticated",
    )


def _probe_contracts(text: str, errors: list[str]) -> None:
    wrapper = _section(
        text,
        PROBE,
        errors,
        "pub fn run_kagemusha_k17_shape_probe_v5(",
        "fn run_kagemusha_k17_shape_probe_in_pool_v5(",
    )
    _require(
        wrapper,
        PROBE,
        errors,
        ".num_threads(KAGEMUSHA_GENERATION_RAYON_THREADS_V5)",
        "pool.install(move ||",
    )
    body = _section(text, PROBE, errors, "fn run_kagemusha_k17_shape_probe_in_pool_v5(")
    _require(body, PROBE, errors, "KagemushaK17ShapeProbeScopeV5::enter()")
    if body.count("halo2_proofs::release_allocator_slack();") < 2:
        errors.append(f"{PROBE}: shape probe must release allocator slack at least twice")
    iteration = _section(
        text,
        PROBE,
        errors,
        "fn kagemusha_k17_shape_probe_iteration_v5(",
        "pub fn run_kagemusha_k17_audit_inventory_probe_v6(",
    )
    _ordered(
        iteration,
        PROBE,
        errors,
        "halo2_proofs::release_allocator_slack();",
        "let step_eq = build_kagemusha_step_eq_circuit_v5(",
    )
    _ordered(
        iteration,
        PROBE,
        errors,
        "KagemushaK17ProbeIterationOutcomeV5::AuditInventory",
        "let step_eq = build_kagemusha_step_eq_circuit_v5(",
    )
    audit = _section(
        text,
        PROBE,
        errors,
        "pub fn run_kagemusha_k17_audit_inventory_probe_v6(",
        "pub fn run_kagemusha_k17_shape_probe_v5(",
    )
    _require(
        audit,
        PROBE,
        errors,
        ".num_threads(KAGEMUSHA_GENERATION_RAYON_THREADS_V5)",
        "KagemushaK17ShapeProbeScopeV5::enter()",
        "populated_step_circuits=0",
    )


def _serialized_builder_contracts(text: str, adapter: str, errors: list[str]) -> None:
    induction = _section(
        text,
        BUILDER,
        errors,
        "fn constrain_kagemusha_serialized_parent_induction_v7",
        "fn constrain_kagemusha_serialized_challenge_and_native_evaluation_v7",
    )
    _require(
        induction,
        BUILDER,
        errors,
        "for value in &parent.instance_cells",
        "Existing(parent.present), Existing(*value)",
        "ctx.constrain_equal(&selected, value)",
    )
    _ordered(
        induction,
        BUILDER,
        errors,
        "for value in &parent.instance_cells",
        "slots.push(KagemushaAssignedParentSlotV7",
    )
    verifier = _section(
        text,
        BUILDER,
        errors,
        "fn verify_kagemusha_serialized_atomic_pair_v7",
        "fn verify_kagemusha_serialized_null_parent_pair_v7",
    )
    _count(verifier, BUILDER, errors, "verify_and_decide_eq_accumulation_v4", 1)
    _count(verifier, BUILDER, errors, "verify_and_decide_ep_accumulation_v4", 1)
    _count(
        verifier,
        BUILDER,
        errors,
        "KagemushaIpaAccumulationProofV4::initialization(manifest.k)",
        2,
    )
    _ordered(
        verifier,
        BUILDER,
        errors,
        "public challenge is not commitment-derived",
        "verify_and_decide_eq_accumulation_v4",
        "verify_and_decide_ep_accumulation_v4",
        "Ok(KagemushaSerializedVerifiedPairV7",
        reverse_last=True,
    )
    creator = _section(text, BUILDER, errors, "fn create_kagemusha_serialized_atomic_pair_once_v7")
    _count(creator, BUILDER, errors, ".serialized_jobs.precommitment(", 4)
    _count(creator, BUILDER, errors, ".serialized_jobs.capacity_profile()?", 4)
    for parity, label in (("eq", "Eq"), ("ep", "Ep")):
        placeholder = f"let (step_{parity}_commitment,"
        key_load = f"let step_{parity}_prepared_key = load_step_{parity}_proving_key()?;"
        final_build = (
            f"let (step_{parity}_circuit, step_{parity}_instances) = "
            f"build_step_{parity}(join)?;"
        )
        count_recapture = f"Kagemusha V7 {label} placeholder/final serialized counts drifted"
        instance_snapshot = f"let step_{parity}_final_cells ="
        instance_invariance = (
            f"&step_{parity}_placeholder_cells,\n        &step_{parity}_final_cells"
        )
        final_precommit = f"let step_{parity}_final_commitment ="
        invariant = f"Kagemusha V7 {label} placeholder/final precommitment invariance failed"
        _ordered(
            creator,
            BUILDER,
            errors,
            placeholder,
            key_load,
            final_build,
            count_recapture,
            final_precommit,
            invariant,
        )
        _ordered(creator, BUILDER, errors, final_build, instance_snapshot, instance_invariance)
        start = creator.find(placeholder)
        key = creator.find(key_load)
        finish = creator.find(invariant)
        precommit = creator.find(final_precommit)
        if min(start, key, finish, precommit) >= 0:
            _require(
                creator[start:key],
                BUILDER,
                errors,
                f"step_{parity}_params,",
                f"step_{parity}_precommit_verifying_key,",
                f"iroha_crypto::rng_from_seed_slice(&step_{parity}_seed.0)",
            )
            _require(
                creator[precommit:finish],
                BUILDER,
                errors,
                (
                    f"step_{parity}_params,\n        "
                    f"step_{parity}_precommit_verifying_key"
                ),
                f"iroha_crypto::rng_from_seed_slice(&step_{parity}_seed.0)",
            )
        _count(
            creator,
            BUILDER,
            errors,
            f"iroha_crypto::rng_from_seed_slice(&step_{parity}_seed.0)",
            2,
        )
        _ordered(
            creator,
            BUILDER,
            errors,
            invariant,
            f"let (step_{parity}_proof, step_{parity}_verifying_key) =",
        )
        proof = creator.find(f"let (step_{parity}_proof, step_{parity}_verifying_key) =")
        if proof >= 0:
            _require(creator[proof:], BUILDER, errors, f"step_{parity}_prepared_key.key")

    geometry = _section(
        text,
        BUILDER,
        errors,
        "fn validate_kagemusha_serialized_vk_geometry_v7",
        "fn succinct_verify_kagemusha_serialized_eq_v7",
    )
    _require(
        geometry,
        BUILDER,
        errors,
        "actual_phases.as_slice() != expected_phases.as_slice()",
        "u32::try_from(actual_rank).ok() != Some(expected_rank)",
        "!selected_is_permuted_advice",
        "constraints.permutation().get_columns().len()",
        "constraints.blinding_factors()",
        "actual_proof_bytes != expected_proof_bytes",
    )
    for function, end, parity in (
        (
            "fn succinct_verify_kagemusha_serialized_eq_v7",
            "fn succinct_verify_kagemusha_serialized_ep_v7",
            "StepEq",
        ),
        (
            "fn succinct_verify_kagemusha_serialized_ep_v7",
            "fn kagemusha_serialized_instance_u128_v7",
            "StepEp",
        ),
    ):
        parity_verifier = _section(text, BUILDER, errors, function, end)
        _ordered(
            parity_verifier,
            BUILDER,
            errors,
            "validate_kagemusha_serialized_vk_geometry_v7(",
            f"KagemushaPastaCycleParityV1::{parity}",
            "kagemusha_selected_advice_commitment_v7(",
        )
    matcher = _section(
        adapter,
        ADAPTER,
        errors,
        "let shape_probe_role = if kagemusha_k17_shape_probe_is_active_v5()",
        "if let Some(role) = shape_probe_role",
    )
    for role in ("StepEqLive", "StepEpLive"):
        _count(text, BUILDER, errors, f'"{role}"', 1)
        _require(matcher, ADAPTER, errors, f'"{role}" => "{role}"')
    _forbid(
        text,
        BUILDER,
        errors,
        '"StepEq live"',
        '"StepEp live"',
        '"StepEqNullParent"',
        '"StepEpNullParent"',
    )


def _release_contracts(text: str, errors: list[str]) -> None:
    execute = _section(text, RELEASE, errors, "fn execute_kagemusha_serialized_release_proof_v7()")
    _ordered(
        execute,
        RELEASE,
        errors,
        "start_kagemusha_generation_memory_guard_v4(Some(",
        "KAGEMUSHA_GENERATION_REVIEWED_MAX_ESTIMATED_BYTES_V5",
        "rayon::ThreadPoolBuilder::new()",
        ".num_threads(KAGEMUSHA_GENERATION_RAYON_THREADS_V5)",
        "pool.install(move || execute_kagemusha_serialized_release_proof_in_pool_v7",
        "prepare_kagemusha_serialized_release_material_v7(",
    )
    _forbid(text, RELEASE, errors, ".effective_memory_limit_bytes()\n        .min(")
    wire = _section(
        text,
        RELEASE,
        errors,
        "struct KagemushaSerializedReleaseCarrierWireV7",
        "impl KagemushaSerializedReleaseCarrierWireV7",
    )
    _require(
        wire,
        RELEASE,
        errors,
        "manifest_sha256: [u8; 32]",
        "step_eq_instances: [u128; KAGEMUSHA_SERIALIZED_PUBLIC_INSTANCE_CELLS_V7]",
        "step_ep_instances: [u128; KAGEMUSHA_SERIALIZED_PUBLIC_INSTANCE_CELLS_V7]",
        "payload: Vec<u8>",
    )
    _forbid(wire, RELEASE, errors, "step_eq_lineage:", "step_ep_lineage:")
    _require(
        text,
        RELEASE,
        errors,
        "norito::encode_canonical(&wire)",
        "KAGEMUSHA_RECURSIVE_SPEND_PROOF_PAIR_RELEASE_MAX_BYTES_V4",
        "canonical_carrier_bytes",
        "raw_proof_pair_bytes",
        "carrier host/public instance splice detected",
        "null_carrier_bytes",
        "null_carrier_sha256",
    )
    _forbid(text, RELEASE, errors, "KAGEMUSHA_SERIALIZED_PAIR_BYTES_V7")
    live = _section(
        text,
        RELEASE,
        errors,
        "    fn authenticate(\n",
        "struct KagemushaSerializedReleaseCarrierV7",
    )
    _require(
        live,
        RELEASE,
        errors,
        "verify_kagemusha_serialized_atomic_pair_v7(",
        "verify_and_decide_eq_accumulation_v4(",
        "verify_and_decide_ep_accumulation_v4(",
    )
    identity = _section(
        text,
        RELEASE,
        errors,
        "fn kagemusha_serialized_carrier_bundle_digest_v7(",
        "fn prepare_kagemusha_serialized_case_recursions_v7(",
    )
    _require(
        identity,
        RELEASE,
        errors,
        "validate_kagemusha_serialized_release_carrier_seal_v7(",
        "Ok(carrier.canonical_sha256)",
    )
    recursion = _section(
        text,
        RELEASE,
        errors,
        "fn prepare_kagemusha_serialized_case_recursions_v7(",
        "fn assert_kagemusha_serialized_live_mutations_fail_v7(",
    )
    _require(recursion, RELEASE, errors, "kagemusha_serialized_parent_slots_digest_v7(")
    null_wire = _section(
        text,
        RELEASE,
        errors,
        "struct KagemushaSerializedNullCarrierWireV7",
        "impl KagemushaSerializedNullCarrierWireV7",
    )
    _require(
        null_wire,
        RELEASE,
        errors,
        "manifest_sha256: [u8; 32]",
        "payload: Vec<u8>",
        "step_eq_proof: &'a [u8]",
        "step_ep_proof: &'a [u8]",
        "step_eq_post_proof_fold: &'a [u8]",
        "step_ep_post_proof_fold: &'a [u8]",
        "step_eq_branch_merge_fold: &'a [u8]",
        "step_ep_branch_merge_fold: &'a [u8]",
    )
    null_auth = _section(
        text,
        RELEASE,
        errors,
        "fn authenticate_kagemusha_serialized_null_carrier_v7(",
        "fn seal_kagemusha_serialized_null_carrier_v7(",
    )
    _require(
        null_auth,
        RELEASE,
        errors,
        "succinct_verify_kagemusha_serialized_eq_v7(",
        "succinct_verify_kagemusha_serialized_ep_v7(",
        "for proof in [&step_eq_post, &step_eq_branch]",
        "for proof in [&step_ep_post, &step_ep_branch]",
    )
    _count(null_auth, RELEASE, errors, "verify_and_decide_eq_accumulation_v4(", 1)
    _count(null_auth, RELEASE, errors, "verify_and_decide_ep_accumulation_v4(", 1)


def _inventory_and_callers(texts: dict[str, str], errors: list[str]) -> None:
    retired = "probe-compact-k17-ipa-audit-bridge"
    _forbid(texts[BENCHMARK], BENCHMARK, errors, retired)
    _forbid(texts[BENCHMARK_WRAPPER], BENCHMARK_WRAPPER, errors, retired)
    _forbid(
        texts[FACADE],
        FACADE,
        errors,
        "run_kagemusha_k17_ipa_audit_bridge_probe_v7",
    )
    _require(
        texts[BRIDGE],
        BRIDGE,
        errors,
        "KAGEMUSHA_SERIALIZED_BRIDGE_REVIEWED_V7: bool = false",
        "if !KAGEMUSHA_SERIALIZED_BRIDGE_REVIEWED_V7",
    )
    _count(
        texts[VECTOR],
        VECTOR,
        errors,
        (
            "impl<F: ff::PrimeField> super::kagemusha_serialized_audit_v7::"
            "KagemushaNativeAuditVectorSourceV7<F>"
        ),
        1,
    )
    _count(
        texts[VECTOR],
        VECTOR,
        errors,
        "KagemushaFrozenNativeAuditVectorV7::from_reviewed_source(",
        1,
    )
    _count(
        texts[VECTOR],
        VECTOR,
        errors,
        "KagemushaReviewedNativeAuditSourceV7(elements)",
        1,
    )
    _count(
        texts[SERIALIZED],
        SERIALIZED,
        errors,
        "pub(super) fn from_reviewed_source<S>(",
        1,
    )

    adapter = texts[ADAPTER]
    verifier_signature = _section(
        adapter,
        ADAPTER,
        errors,
        "pub fn verify_candidate_recursive_step_two_receipt_v4",
        "where",
    )
    generator_signature = _section(
        adapter,
        ADAPTER,
        errors,
        "pub fn generate_candidate_recursive_step_two_receipt_v4",
        "where",
    )
    _require(verifier_signature, ADAPTER, errors, "qualification_memory_contract")
    _require(generator_signature, ADAPTER, errors, "memory_guard")
    for relative in (BUNDLE, CATALOG, KAGAMI):
        source = texts[relative]
        call_parts = source.split("verify_candidate_recursive_step_two_receipt_v4(")[1:]
        if len(call_parts) != 1:
            errors.append(f"{relative}: unexpected qualification verifier caller count")
        elif "qualification_memory_contract" not in call_parts[0][:800]:
            errors.append(f"{relative}: verifier caller lacks explicit memory contract")
    _count(
        texts[BUNDLE],
        BUNDLE,
        errors,
        "start_kagemusha_generation_memory_guard_v4(",
        1,
    )
    for operator in (
        "fn build_candidate(",
        "fn publish_staged_candidate(",
        "fn validate_candidate(",
        "fn finalize_release(",
    ):
        signature = _section(texts[BUNDLE], BUNDLE, errors, operator, ") ->")
        _require(signature, BUNDLE, errors, "memory_guard")
    _count(
        texts[KAGAMI],
        KAGAMI,
        errors,
        "start_kagemusha_generation_memory_guard_v4(",
        2,
    )
    _count(
        texts[CATALOG],
        CATALOG,
        errors,
        "KagemushaQualificationMemoryContractV4::for_runtime_catalog(",
        1,
    )


def recursion_source_contract_errors(
    root: Path, overrides: dict[str, str] | None = None
) -> list[str]:
    """Return deterministic source-contract diagnostics for the reviewed tree."""
    errors: list[str] = []
    overrides = overrides or {}
    paths = (
        ADAPTER,
        GENERATED,
        PROBE,
        BUILDER,
        RELEASE,
        BRIDGE,
        VECTOR,
        SERIALIZED,
        BENCHMARK,
        BENCHMARK_WRAPPER,
        FACADE,
        BUNDLE,
        CATALOG,
        KAGAMI,
    )
    texts = {path: _read(root, path, overrides, errors) for path in paths}
    generated_include = 'include!("kagemusha_recursion_adapter/generated_artifacts.rs");'
    if texts[ADAPTER].count(generated_include) != 1:
        errors.append(f"{ADAPTER}: expected exactly one generated-artifacts include")
    else:
        texts[ADAPTER] = texts[ADAPTER].replace(
            generated_include, texts[GENERATED], 1
        )
    provider = overrides.get(PROVIDER, _SOURCE)
    _require(
        provider,
        PROVIDER,
        errors,
        '_KAGEMUSHA_RECURSION_SOURCE_CONTRACT_CONTEXT_V1',
    )
    for function in (
        "recursion_source_contract_errors",
        "_adapter_contracts",
        "_serialized_builder_contracts",
        "_release_contracts",
    ):
        if len(re.findall(rf"(?m)^def {re.escape(function)}\(", provider)) != 1:
            errors.append(f"{PROVIDER}: expected exactly one {function} definition")
    for relative, text in (*texts.items(), (PROVIDER, provider)):
        if re.search(r"(?m)^(?:<<<<<<<(?: .*)?|=======|>>>>>>>(?: .*)?)$", text):
            errors.append(f"{relative}: unresolved Git merge conflict marker")
    _adapter_contracts(texts[ADAPTER], errors)
    _probe_contracts(texts[PROBE], errors)
    _serialized_builder_contracts(texts[BUILDER], texts[ADAPTER], errors)
    _release_contracts(texts[RELEASE], errors)
    _inventory_and_callers(texts, errors)
    return errors


def _main(arguments: list[str]) -> int:
    root = Path(__file__).resolve().parents[1]
    errors = recursion_source_contract_errors(root)
    if "--self-test" in arguments:
        adapter = (root / ADAPTER).read_text(encoding="utf-8")
        needle = "compact.len() != expected_compact_len"
        hostile = adapter.replace(needle, "compact.len() != 64", 1)
        negative = recursion_source_contract_errors(root, {ADAPTER: hostile})
        if not negative:
            errors.append("self-test failed to reject compact-header length substitution")
    if errors:
        for error in errors:
            print(error, file=sys.stderr)
        return 1
    print("Kagemusha recursion source contracts passed")
    return 0


if __name__ == "__main__" and not _IN_GATE:
    raise SystemExit(_main(sys.argv[1:]))
