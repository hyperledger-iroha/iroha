/// IVM integration helpers.
///
/// This module currently only exposes a runtime cache used by other
/// components for the Iroha Virtual Machine (IVM).
pub mod cache;
/// Host adapter for IVM. See module docs for design and current limitations.
pub mod host;
/// Effective restrictions for separately selected named contract host profiles.
pub(crate) mod host_profile;
/// Resolution of the execution profile of an artifact, contract or invocation.
pub(crate) mod profile;
/// Projection of internal VM failures onto the wire error types, keyed by engine.
pub(crate) mod projection;
/// Exact, privacy-safe public return decoding.
pub mod return_value;
use crate::state::WorldReadOnly;
use iroha_crypto::Hash;
use iroha_data_model::{
    ValidationFail,
    executor::{
        ContractRejection, IvmAdmissionError, ManifestAbiHashMismatchInfo,
        ManifestCodeHashMismatchInfo, MaxCyclesExceedsUpperBoundInfo,
    },
    smart_contract::manifest::ContractManifest,
};
use iroha_model_base::metadata::Metadata;
use mv::storage::StorageReadOnly;
use std::{collections::BTreeSet, num::NonZeroU64};
/// Convert deterministic program preparation failures into public admission errors.
///
/// The projection belongs to the engine that raised the failure
/// ([`projection::admission_error`]). This entry point has no profile context and applies the
/// engine of the first-release interface profile.
// TODO: Callers that hold a resolved profile pass its engine to `projection::admission_error`.
#[must_use]
pub(crate) fn admission_reason_from_vm_error(error: ivm::VMError) -> IvmAdmissionError {
    projection::admission_error(
        profile::ResolvedIvmProfile::interface_default().engine(),
        error,
    )
}
/// Convert deterministic program preparation failures into public admission errors.
#[must_use]
pub(crate) fn program_admission_error(error: ivm::VMError) -> ValidationFail {
    ValidationFail::IvmAdmission(admission_reason_from_vm_error(error))
}
/// Reject contract/deployment metadata on a contract-less generic program.
///
/// Presence is checked before decoding so malformed values cannot evade the
/// discriminator and every execution path observes identical precedence.
pub(crate) fn validate_generic_execution_metadata(
    metadata: &Metadata,
) -> Result<(), ValidationFail> {
    const RESERVED: [&str; 7] = [
        "contract_manifest",
        "gov_contract_address",
        "gov_manifest_approvers",
        "contract_address",
        "contract_alias",
        "contract_entrypoint",
        "contract_payload",
    ];
    if let Some(key) = RESERVED
        .into_iter()
        .find(|key| metadata.get(*key).is_some())
    {
        return Err(ValidationFail::NotPermitted(format!(
            "generic IVM programs cannot carry reserved `{key}` metadata"
        )));
    }
    Ok(())
}
/// Reject every contract identity shape for a contract-less generic program.
///
/// Metadata and the content-addressed manifest registry are checked together so the same hash
/// cannot execute as generic bytecode on direct/trigger paths while overlay admission classifies
/// it as deployed contract code. Values remain unparsed: mere presence of reserved metadata fails
/// closed.
pub(crate) fn validate_generic_execution_context(
    world: &impl WorldReadOnly,
    metadata: &Metadata,
    artifact_id: iroha_data_model::smart_contract::ContractArtifactId,
) -> Result<(), ValidationFail> {
    validate_generic_execution_metadata(metadata)?;
    let scope = crate::sumeragi::lanes::routing::committed_root_scope(world).ok_or_else(|| {
        ValidationFail::NotPermitted("generic IVM requires immutable root scope".into())
    })?;
    if !matches!(
        scope,
        iroha_data_model::block::consensus::SumeragiRootScope::Global
    ) {
        return Err(ValidationFail::NotPermitted(
            "private roots require address-bound contract execution".into(),
        ));
    }
    if world.contract_manifests().get(&artifact_id).is_some() {
        return Err(ValidationFail::NotPermitted(
            "generic IVM program hash is bound to a contract manifest in live state".to_owned(),
        ));
    }
    Ok(())
}
/// ABI hashes of the host profiles the root's selection activates (`live_runtime_upgrades.md`
/// §3.1): a contract executes only under a host profile its root selected, so admission compares
/// its ABI hash with these. The baseline activates exactly the ABI V1 host profile.
///
/// # Errors
/// The selection names a host profile its World has not published.
pub(crate) fn active_host_abi_hashes(
    world: &impl WorldReadOnly,
) -> Result<BTreeSet<Hash>, IvmAdmissionError> {
    let selection = world.runtime_selection();
    let definitions = world.runtime_definitions();
    let mut hashes = BTreeSet::new();
    for id in &selection.active {
        match definitions.get(id) {
            Some(iroha_data_model::runtime::definition::DefinitionDescriptorV1::HostProfile(
                profile,
            )) => {
                hashes.insert(Hash::prehashed(profile.abi_hash));
            }
            Some(_) => {}
            None => return Err(IvmAdmissionError::ManifestMalformed),
        }
    }
    Ok(hashes)
}
/// Validate the consensus-binding hashes carried by a contract manifest.
///
/// V1 manifests must bind both the complete artifact and the exact ABI descriptor. Missing fields
/// are rejected before mismatches, and the code hash is checked before the ABI hash to give every
/// node identical error precedence.
pub(crate) fn validate_manifest_hashes(
    manifest: &ContractManifest,
    actual_code_hash: Hash,
    actual_abi_hash: Hash,
) -> Result<(), IvmAdmissionError> {
    let expected_code_hash = manifest
        .code_hash
        .ok_or(IvmAdmissionError::ManifestCodeHashMissing)?;
    if expected_code_hash != actual_code_hash {
        return Err(IvmAdmissionError::ManifestCodeHashMismatch(
            ManifestCodeHashMismatchInfo {
                expected: expected_code_hash,
                actual: actual_code_hash,
            },
        ));
    }
    let expected_abi_hash = manifest
        .abi_hash
        .ok_or(IvmAdmissionError::ManifestAbiHashMissing)?;
    if expected_abi_hash != actual_abi_hash {
        return Err(IvmAdmissionError::ManifestAbiHashMismatch(
            ManifestAbiHashMismatchInfo {
                expected: expected_abi_hash,
                actual: actual_abi_hash,
            },
        ));
    }
    Ok(())
}
/// Validate the ZK trace bound an artifact declares against node policy.
///
/// Gas is the only execution budget and the caller supplies it; the header field `max_cycles` is
/// the trace bound of a ZK-mode artifact alone. Header admission already requires it to be
/// non-zero exactly for ZK headers ([`validate_header_policy`] repeats that rule). This check
/// bounds a ZK artifact's trace by the committed `pipeline.ivm_max_cycles_upper_bound`, and every
/// path that executes or persists an artifact shares it, so an artifact cannot be stored for later
/// execution with a trace bound above the ceiling. A non-ZK artifact has no cycle check.
///
/// [`validate_header_policy`]: crate::pipeline::overlay::validate_header_policy
pub(crate) fn validate_zk_trace_bound(
    meta: &ivm::ProgramMetadata,
    upper_bound: NonZeroU64,
) -> Result<(), IvmAdmissionError> {
    if meta.mode & ivm::ivm_mode::ZK == 0 {
        return Ok(());
    }
    let cycles = NonZeroU64::new(meta.max_cycles).ok_or(IvmAdmissionError::MissingMaxCycles)?;
    if cycles > upper_bound {
        return Err(IvmAdmissionError::MaxCyclesExceedsUpperBound(
            MaxCyclesExceedsUpperBoundInfo {
                max_cycles: cycles.get(),
                upper_bound: upper_bound.get(),
            },
        ));
    }
    Ok(())
}
/// Validate the cycle limit a header declares against the committed ceiling, under the rules of
/// the engine that admitted it.
///
/// Every path that persists an artifact for later execution shares this check, so an artifact
/// cannot be stored with a limit above the ceiling.
///
/// - Baseline: every artifact declares a positive `max_cycles` within the ceiling (the anchor
///   Core's `validate_cycle_ceiling`).
/// - C10: only a ZK artifact's trace bound is checked ([`validate_zk_trace_bound`]).
///
/// # Errors
/// The first violated rule of that engine.
pub(crate) fn validate_cycle_ceiling_for(
    engine: ivm::IvmEngine,
    meta: &ivm::ProgramMetadata,
    upper_bound: NonZeroU64,
) -> Result<(), IvmAdmissionError> {
    match engine {
        ivm::IvmEngine::Baseline => {
            ivm_abi::baseline::validate_cycle_ceiling(meta, upper_bound).map(drop)
        }
        ivm::IvmEngine::C10 => validate_zk_trace_bound(meta, upper_bound),
    }
}
/// The header-derived cycle budget of an execution, under the rules of the engine that admitted
/// the program.
///
/// - Baseline: the anchor Core's `validate_cycle_limits`, governance `fuel` before the ceiling;
///   the budget is the header's `max_cycles`. It bounds the run's cycles and, through
///   [`gas_limit_for_cycles`], its gas where the anchor applied that bound.
/// - C10: gas, supplied by the caller, is the only budget. Only a ZK trace bound is checked and
///   no cycle budget results.
///
/// # Errors
/// The first violated rule of that engine.
pub(crate) fn execution_cycle_budget_for(
    engine: ivm::IvmEngine,
    meta: &ivm::ProgramMetadata,
    upper_bound: NonZeroU64,
    fuel: NonZeroU64,
) -> Result<Option<NonZeroU64>, IvmAdmissionError> {
    match engine {
        ivm::IvmEngine::Baseline => {
            ivm_abi::baseline::validate_cycle_limits(meta, upper_bound, fuel).map(Some)
        }
        ivm::IvmEngine::C10 => validate_zk_trace_bound(meta, upper_bound).map(|()| None),
    }
}
/// Gas limit a baseline cycle budget grants: the cycles times the baseline's worst-case
/// instruction cost (the anchor Core's conversion).
///
/// The baseline interpreter pads traces to exactly `max_cycles` when cycle limits apply, charging
/// one unit of gas per padded cycle in addition to the per-instruction schedule, so the worst-case
/// instruction cost is the multiplier. A positive budget is a [`NonZeroU64`], so this never
/// manufactures an unbounded limit.
#[must_use]
pub fn gas_limit_for_cycles(cycles: NonZeroU64) -> u64 {
    cycles
        .get()
        .saturating_mul(ivm::gas::baseline::max_instruction_cost())
}
/// Classify an original VM failure before rendering completed semantic rejection context.
///
/// Local refusals retain their original release owner and never enter diagnostic
/// formatting. Output allocation for completed validation messages remains separate.
#[must_use]
pub fn map_vm_error_with_context_to_validation(
    vm: &ivm::IVM,
    error: ivm::VMError,
) -> crate::execution_attempt::ExecutionAttemptError<ValidationFail> {
    // The engine of the VM that ran owns both projections of its failure.
    let engine = vm.engine();
    let fault = vm.execution_fault(
        &error,
        iroha_data_model::executor::fault::IvmFaultPositionV1::ReturnValidation,
    );
    if engine.fault_kind(&error).is_some() && fault.is_none() {
        return crate::execution_attempt::ExecutionAttemptError::Deferred(
            ivm::error::ExecutionDeferral::LocalInvariantViolation.into(),
        );
    }
    crate::execution_attempt::vm_attempt_error(error, |error| {
        // The declared rejection already owns its authenticated strings. Remove
        // metering wrappers only for that variant so all other displays retain
        // their original error context.
        let error = if matches!(error.as_unmetered(), ivm::VMError::ContractAbort { .. }) {
            error.into_unmetered()
        } else {
            error
        };
        if let ivm::VMError::ContractAbort {
            contract,
            name,
            error_type,
            schema_hash,
            code,
            message,
        } = error
        {
            // Transfer the original completed output without cloning its backing.
            return ValidationFail::ContractRejected(ContractRejection {
                contract,
                error_type,
                schema_hash,
                name,
                code,
                message,
            });
        }
        fault.map_or_else(
            || {
                ValidationFail::IvmAdmission(projection::admission_error(
                    engine,
                    error.into_unmetered(),
                ))
            },
            ValidationFail::IvmFault,
        )
    })
}
#[cfg(test)]
#[path = "ivm/diagnostic_tests.rs"]
mod diagnostic_tests;
#[cfg(test)]
mod tests {
    use super::*;
    use iroha_data_model::smart_contract::manifest::ContractManifest;
    use iroha_model_base::metadata::Metadata;
    use iroha_model_base::name::Name;
    use iroha_primitives::json::Json;
    fn manifest_with_hashes(code_hash: Option<Hash>, abi_hash: Option<Hash>) -> ContractManifest {
        ContractManifest {
            events: Vec::new(),
            enum_types: Vec::new(),
            permissions: Vec::new(),
            seiyaku_name: None,
            code_hash,
            abi_hash,
            compiler_fingerprint: None,
            features_bitmap: None,
            access_set_hints: None,
            entrypoints: None,
            states: None,
            kotoba: None,
            error_messages: None,
            error_types: None,
            provenance: None,
        }
    }
    #[test]
    fn generic_programs_reject_every_contract_metadata_shape_without_decoding_values() {
        for key in [
            "contract_manifest",
            "gov_contract_address",
            "gov_manifest_approvers",
            "contract_address",
            "contract_alias",
            "contract_entrypoint",
            "contract_payload",
        ] {
            let mut metadata = Metadata::default();
            metadata.insert(
                key.parse::<Name>().expect("static metadata key"),
                Json::new("malformed-reserved-value"),
            );
            let error = validate_generic_execution_metadata(&metadata)
                .expect_err("generic programs must reject contract/deployment metadata");
            assert!(
                matches!(error, ValidationFail::NotPermitted(ref message) if message.contains(key)),
                "unexpected rejection for `{key}`: {error}"
            );
        }
    }
    #[test]
    fn generic_program_hash_rejects_live_manifest_binding() {
        let code_hash = Hash::new(b"generic-binding-probe");
        let metadata = Metadata::default();
        let mut world = crate::sumeragi::lanes::routing::test_support::world(
            iroha_data_model::block::consensus::SumeragiRootScope::Global,
        );
        let artifact_id = iroha_data_model::smart_contract::ContractArtifactId::new(
            iroha_model_base::topology::DataSpaceId::UNIVERSAL,
            code_hash,
        );
        validate_generic_execution_context(&world.view(), &metadata, artifact_id)
            .expect("unbound generic program is valid");
        world.contract_manifests.insert(
            artifact_id,
            manifest_with_hashes(Some(code_hash), Some(Hash::new(b"generic-abi"))),
        );
        let error = validate_generic_execution_context(&world.view(), &metadata, artifact_id)
            .expect_err("a manifest-bound hash is not generic");
        assert!(error.to_string().contains("contract manifest"));
    }
    #[test]
    fn generic_artifact_classification_is_scoped_and_requires_a_global_root() {
        use iroha_data_model::{
            block::consensus::SumeragiRootScope, smart_contract::ContractArtifactId,
        };
        use iroha_model_base::topology::DataSpaceId;
        let hash = Hash::new(b"same-bytecode-distinct-dataspaces");
        let own = ContractArtifactId::new(DataSpaceId::new((1_u64 << 40) + 7), hash);
        let foreign = ContractArtifactId::new(DataSpaceId::new(7), hash);
        let metadata = Metadata::default();
        let mut world =
            crate::sumeragi::lanes::routing::test_support::world(SumeragiRootScope::Global);
        world.contract_manifests.insert(
            foreign,
            manifest_with_hashes(Some(hash), Some(Hash::new(b"abi"))),
        );
        assert!(validate_generic_execution_context(&world.view(), &metadata, own).is_ok());
        assert!(validate_generic_execution_context(&world.view(), &metadata, foreign).is_err());
        let unbound = crate::state::World::new();
        assert!(validate_generic_execution_context(&unbound.view(), &metadata, own).is_err());
        let private =
            crate::sumeragi::lanes::routing::test_support::world(SumeragiRootScope::Dataspace {
                parent_network_id: iroha_data_model::NetworkId::from_genesis_hash(
                    iroha_crypto::HashOf::from_untyped_unchecked(Hash::new(b"parent")),
                ),
                dataspace_id: own.dataspace_id,
            });
        assert!(validate_generic_execution_context(&private.view(), &metadata, own).is_err());
    }
    #[test]
    fn active_host_abi_hashes_come_from_the_committed_selection() {
        let world = crate::state::World::new();
        assert_eq!(
            active_host_abi_hashes(&world.view()),
            Ok(BTreeSet::from([Hash::prehashed(
                ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1)
            )])),
            "the baseline selection activates exactly the first-release host profile"
        );
    }
    #[test]
    fn manifest_hash_validation_is_complete_and_has_stable_precedence() {
        let code_hash = Hash::new(b"manifest-code");
        let abi_hash = Hash::new(b"manifest-abi");
        let wrong_code_hash = Hash::new(b"wrong-code");
        let wrong_abi_hash = Hash::new(b"wrong-abi");
        assert!(matches!(
            validate_manifest_hashes(&manifest_with_hashes(None, None), code_hash, abi_hash),
            Err(IvmAdmissionError::ManifestCodeHashMissing)
        ));
        assert!(matches!(
            validate_manifest_hashes(
                &manifest_with_hashes(Some(code_hash), None),
                code_hash,
                abi_hash
            ),
            Err(IvmAdmissionError::ManifestAbiHashMissing)
        ));
        assert!(matches!(
            validate_manifest_hashes(
                &manifest_with_hashes(Some(wrong_code_hash), Some(wrong_abi_hash)),
                code_hash,
                abi_hash
            ),
            Err(IvmAdmissionError::ManifestCodeHashMismatch(info))
                if info.expected == wrong_code_hash && info.actual == code_hash
        ));
        assert!(matches!(
            validate_manifest_hashes(
                &manifest_with_hashes(Some(code_hash), Some(wrong_abi_hash)),
                code_hash,
                abi_hash
            ),
            Err(IvmAdmissionError::ManifestAbiHashMismatch(info))
                if info.expected == wrong_abi_hash && info.actual == abi_hash
        ));
        assert_eq!(
            validate_manifest_hashes(
                &manifest_with_hashes(Some(code_hash), Some(abi_hash)),
                code_hash,
                abi_hash
            ),
            Ok(())
        );
    }
    #[test]
    fn header_cycle_bound_errors_map_to_their_admission_reasons() {
        // C10 header rules are projected by the C10 engine; the restored baseline enum carries
        // the trace bound without ZK mode by its text.
        let c10 = |error| projection::admission_error(ivm::IvmEngine::C10, error);
        assert_eq!(
            c10(ivm::VMError::MissingMaxCycles),
            IvmAdmissionError::MissingMaxCycles
        );
        assert!(matches!(
            c10(ivm::VMError::MaxCyclesWithoutZk { max_cycles: 7 }),
            IvmAdmissionError::BytecodeDecodingFailed(message) if message.contains('7')
        ));
        let zk_without_bound = ivm::ProgramMetadata {
            mode: ivm::ivm_mode::ZK,
            ..ivm::ProgramMetadata::default()
        }
        .encode();
        let error = crate::ivm_test_support::parse_declared(&zk_without_bound)
            .expect_err("a ZK header requires a trace bound");
        assert_eq!(c10(error), IvmAdmissionError::MissingMaxCycles);
        let bound_without_zk = ivm::ProgramMetadata {
            max_cycles: 9,
            ..ivm::ProgramMetadata::default()
        }
        .encode();
        let error = crate::ivm_test_support::parse_declared(&bound_without_zk)
            .expect_err("a non-ZK header declares no trace bound");
        assert!(matches!(
            c10(error),
            IvmAdmissionError::BytecodeDecodingFailed(message) if message.contains("trace bound 9")
        ));
    }
    #[test]
    fn baseline_cycle_rules_are_the_anchor_rules() {
        use iroha_data_model::executor::MaxCyclesExceedsFuelInfo;
        let engine = ivm::IvmEngine::Baseline;
        let upper_bound = NonZeroU64::new(42).expect("test ceiling is non-zero");
        let fuel = NonZeroU64::new(40).expect("test fuel is non-zero");
        let mut metadata = ivm::ProgramMetadata::default();
        // A baseline header without a cycle limit is refused whatever its mode.
        assert_eq!(
            validate_cycle_ceiling_for(engine, &metadata, upper_bound),
            Err(IvmAdmissionError::MissingMaxCycles)
        );
        assert_eq!(
            execution_cycle_budget_for(engine, &metadata, upper_bound, fuel),
            Err(IvmAdmissionError::MissingMaxCycles)
        );
        metadata.max_cycles = 40;
        assert_eq!(
            execution_cycle_budget_for(engine, &metadata, upper_bound, fuel),
            Ok(NonZeroU64::new(40))
        );
        assert_eq!(
            validate_cycle_ceiling_for(engine, &metadata, upper_bound),
            Ok(())
        );
        // Fuel is checked before the ceiling.
        metadata.max_cycles = 43;
        assert_eq!(
            execution_cycle_budget_for(engine, &metadata, upper_bound, fuel),
            Err(IvmAdmissionError::MaxCyclesExceedsFuel(
                MaxCyclesExceedsFuelInfo {
                    max_cycles: 43,
                    fuel_limit: 40,
                }
            ))
        );
        assert!(matches!(
            validate_cycle_ceiling_for(engine, &metadata, upper_bound),
            Err(IvmAdmissionError::MaxCyclesExceedsUpperBound(info))
                if info.max_cycles == 43 && info.upper_bound == 42
        ));
        // C10 has no cycle budget for a non-ZK program.
        assert_eq!(
            execution_cycle_budget_for(
                ivm::IvmEngine::C10,
                &ivm::ProgramMetadata::default(),
                upper_bound,
                fuel
            ),
            Ok(None)
        );
        let cost = ivm::gas::baseline::max_instruction_cost();
        assert_eq!(cost, 5_000);
        assert_eq!(gas_limit_for_cycles(NonZeroU64::new(2).unwrap()), 2 * cost);
        assert_eq!(gas_limit_for_cycles(NonZeroU64::MAX), u64::MAX);
    }
    #[test]
    fn zk_trace_bound_validation_applies_only_to_zk_artifacts() {
        let upper_bound = NonZeroU64::new(42).expect("test ceiling is non-zero");
        // A non-ZK artifact has no cycle check: gas is its only budget.
        assert_eq!(
            validate_zk_trace_bound(&ivm::ProgramMetadata::default(), upper_bound),
            Ok(())
        );
        let mut metadata = ivm::ProgramMetadata {
            mode: ivm::ivm_mode::ZK,
            max_cycles: 0,
            ..ivm::ProgramMetadata::default()
        };
        assert!(matches!(
            validate_zk_trace_bound(&metadata, upper_bound),
            Err(IvmAdmissionError::MissingMaxCycles)
        ));
        metadata.max_cycles = 42;
        assert_eq!(validate_zk_trace_bound(&metadata, upper_bound), Ok(()));
        metadata.max_cycles = 43;
        assert!(matches!(
            validate_zk_trace_bound(&metadata, upper_bound),
            Err(IvmAdmissionError::MaxCyclesExceedsUpperBound(info))
                if info.max_cycles == 43 && info.upper_bound == 42
        ));
    }
    #[test]
    fn compiler_and_node_release_cycle_defaults_match() {
        // A default (baseline) build's cycle limit and a C10 ZK build's trace bound both fit the
        // default admission ceiling.
        let options = kotodama_lang::compiler::CompilerOptions::default();
        assert_eq!(options.engine, ivm::IvmEngine::Baseline);
        let node_default = iroha_config::parameters::defaults::pipeline::IVM_MAX_CYCLES_UPPER_BOUND;
        assert_eq!(options.baseline_max_cycles, node_default);
        assert_eq!(options.zk_trace_cycles, node_default);
        assert_eq!(node_default.get(), 1_000_000);
    }
    #[test]
    fn vm_error_maps_to_not_permitted() {
        let vm = ivm::IVM::try_new(100).unwrap();
        let err = crate::execution_attempt::expect_completed_rejection(
            map_vm_error_with_context_to_validation(&vm, ivm::VMError::OutOfGas),
        );
        assert!(
            matches!(err, ValidationFail::IvmFault(fault) if fault.kind == iroha_data_model::executor::fault::IvmFaultKindV1::OutOfGas)
        );
    }
    #[test]
    fn declared_contract_abort_maps_to_manifest_authenticated_rejection() {
        let artifact = kotodama_lang::compiler::Compiler::new()
            .compile_source(
                r"
                seiyaku LiquidityPolicy { permission Test;
                    error enum LiquidityError {
                        BelowMinimum = 18,
                    }

                    kotoage fn reject() authorize(Test) {
                        require(false, LiquidityError::BelowMinimum);
                    }
                }
                ",
            )
            .expect("compile contract rejection fixture");
        let mut vm = ivm::IVM::new(u64::MAX);
        vm.load_program_for_profile(
            &artifact,
            crate::ivm_test_support::declared_profile(&artifact),
        )
        .expect("load contract rejection fixture");
        let entry_pc = vm
            .contract_interface()
            .expect("embedded contract interface")
            .entrypoints
            .iter()
            .find(|entrypoint| entrypoint.name == "reject")
            .expect("reject entrypoint")
            .entry_pc;
        let prefix = crate::ivm_test_support::parse_declared(&artifact)
            .unwrap()
            .prefix_len() as u64;
        vm.set_program_counter(prefix + entry_pc)
            .expect("select reject entrypoint");
        let error = vm.run().expect_err("declared require must abort");
        let descriptor = vm
            .contract_interface()
            .expect("interface")
            .error_types
            .iter()
            .find(|descriptor| descriptor.variant(18).is_some())
            .expect("declared error");
        let ivm::VMError::ContractAbort {
            contract,
            name,
            error_type,
            schema_hash,
            code,
            message,
        } = &error
        else {
            panic!("declared require must retain its exact contract rejection: {error:?}");
        };
        assert_eq!(contract.as_ref(), "LiquidityPolicy");
        assert_eq!(name, "BelowMinimum");
        assert_eq!(error_type, &descriptor.identity);
        assert_eq!(*schema_hash, descriptor.schema_hash());
        assert_eq!(*code, 18);
        assert_eq!(*message, None);
        assert_eq!(
            crate::execution_attempt::expect_completed_rejection(
                map_vm_error_with_context_to_validation(&vm, error),
            ),
            ValidationFail::ContractRejected(ContractRejection {
                contract: "LiquidityPolicy".into(),
                error_type: descriptor.identity.clone(),
                schema_hash: descriptor.schema_hash(),
                name: "BelowMinimum".to_owned(),
                code: 18,
                message: None,
            })
        );
    }
}

/// Select only the exact profile advertised by the artifact and active in this root.
/// This is a profile choice, not header admission: the explicit parser still authenticates every
/// header field in its original order. Unselected/unknown hashes retain baseline rejection.
///
/// [`profile::resolve_artifact`] decides; this accessor returns the compiled identity of its
/// result, engine included. Readers outside Core (Torii) parse, verify and prepare stored
/// artifacts under this profile. A named policy alone does not name a profile: C10's own ABI V1
/// surface and the frozen baseline share `SyscallPolicy::AbiV1`.
#[must_use]
pub fn artifact_profile(world: &impl WorldReadOnly, bytes: &[u8]) -> ivm::IvmProfileId {
    profile::resolve_artifact(world, bytes).id()
}
/// Preserve baseline hash acceptance and admit only an exact compiled named profile selected
/// by this root. Runtime execution still enforces the effective restrictions at its block height.
#[must_use]
pub fn known_selected_abi(world: &impl WorldReadOnly, hash: &[u8; 32]) -> bool {
    profile::resolve_abi_hash(world, hash).is_some()
}
/// Verify an original contract under its root's selection using the original State pool.
pub(crate) fn verify_contract_in_world(
    world: &impl WorldReadOnly,
    bytes: &[u8],
    budget: &iroha_allocation::AllocationBudget,
) -> Result<ivm::VerifiedContractArtifact, ivm::ContractArtifactError> {
    ivm::verify_contract_artifact_with_memory_budget_for_profile(
        bytes,
        budget,
        profile::resolve_artifact(world, bytes).id(),
    )
}
