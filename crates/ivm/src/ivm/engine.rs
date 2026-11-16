//! Profile-engine boundary behind the one public [`IVM`] facade.
//!
//! [`IVM`] is the only VM type a host sees. What differs between execution profiles is supplied
//! by a [`ProfileEngine`]: how an artifact is admitted and prepared, how the loaded program
//! runs, how a failure is projected onto the wire, and which metering schedule prices it. The
//! engine of a loaded image is the engine of its resolved profile
//! ([`IvmProfileId::of_policy`] for a named policy). The caller resolves it before preparation; no
//! function here chooses an engine from artifact bytes, and a refusal of one engine is never
//! retried on another.
//!
//! Two engines exist: [`C10Engine`] and the frozen [`BaselineEngine`]. Their machine state
//! (registers, memory, traces) is stored inline in [`IVM`], because hosts and tests across the
//! workspace read those public fields: the register file is sized for the 256-register baseline
//! and the C10 engine addresses its first 32, and the few values only the baseline reads live in
//! [`super::baseline::BaselineMachine`].

use super::*;
use crate::{IvmEngine, IvmProfileId};
use iroha_data_model::executor::fault::IvmFaultKindV1;

/// Operations one engine supplies behind the facade.
pub(super) trait ProfileEngine {
    /// Identity bound into profile and cache identities.
    const ID: IvmEngine;

    /// Admit and prepare `program` under `policy`, installing the image only when a VM is
    /// supplied. Validation-only callers retire the admitted image without mutable VM storage.
    fn prepare(
        program: &[u8],
        destination: Option<&mut IVM>,
        profile: IvmProfileId,
    ) -> Result<(), VMError>;

    /// Execute the loaded program of `vm` from its current program counter.
    fn run(
        vm: &mut IVM,
        host: &mut dyn IVMHost,
        recorder: Option<&mut DiagnosticStepRecorder>,
    ) -> Result<(), VMError>;

    /// Wire fault kind of a failure this engine raised.
    fn fault_kind(error: &VMError) -> Option<IvmFaultKindV1> {
        Self::ID.fault_kind(error)
    }

    /// Hash of the metering schedule that prices this engine's instructions and syscalls.
    fn metering_schedule_hash() -> iroha_crypto::Hash;
}

/// The consolidated interpreter: the engine of the runtime-lifecycle policy and of the explicit
/// [`IvmProfileId::C10_ABI_V1`] profile.
pub(super) struct C10Engine;

impl ProfileEngine for C10Engine {
    const ID: IvmEngine = IvmEngine::C10;

    fn prepare(
        program: &[u8],
        destination: Option<&mut IVM>,
        profile: IvmProfileId,
    ) -> Result<(), VMError> {
        program_load::prepare_c10(program, destination, profile)
    }

    fn run(
        vm: &mut IVM,
        host: &mut dyn IVMHost,
        recorder: Option<&mut DiagnosticStepRecorder>,
    ) -> Result<(), VMError> {
        vm.run_with_host_ref(host, recorder)
    }

    fn metering_schedule_hash() -> iroha_crypto::Hash {
        crate::gas::c10_schedule_hash()
    }
}

/// The frozen pre-C10 interpreter of the installed baseline selection: the engine of
/// [`crate::SyscallPolicy::AbiV1`].
pub(super) struct BaselineEngine;

impl ProfileEngine for BaselineEngine {
    const ID: IvmEngine = IvmEngine::Baseline;

    fn prepare(
        program: &[u8],
        destination: Option<&mut IVM>,
        profile: IvmProfileId,
    ) -> Result<(), VMError> {
        program_load::prepare_baseline(program, destination, profile)
    }

    fn run(
        vm: &mut IVM,
        host: &mut dyn IVMHost,
        recorder: Option<&mut DiagnosticStepRecorder>,
    ) -> Result<(), VMError> {
        vm.run_baseline(host, recorder)
    }

    fn metering_schedule_hash() -> iroha_crypto::Hash {
        crate::gas::baseline::schedule_hash()
    }
}

/// Prepare `program` with the engine of the caller's resolved profile.
pub(super) fn prepare(
    program: &[u8],
    destination: Option<&mut IVM>,
    profile: IvmProfileId,
) -> Result<(), VMError> {
    match profile.engine() {
        IvmEngine::C10 => C10Engine::prepare(program, destination, profile),
        IvmEngine::Baseline => BaselineEngine::prepare(program, destination, profile),
    }
}

/// Run the loaded program with the engine of its installed image.
#[inline]
pub(super) fn run(
    vm: &mut IVM,
    host: &mut dyn IVMHost,
    recorder: Option<&mut DiagnosticStepRecorder>,
) -> Result<(), VMError> {
    match vm.engine() {
        IvmEngine::C10 => C10Engine::run(vm, host, recorder),
        IvmEngine::Baseline => BaselineEngine::run(vm, host, recorder),
    }
}

/// Project a failure raised by `engine` onto the wire fault kind.
pub(super) fn fault_kind(engine: IvmEngine, error: &VMError) -> Option<IvmFaultKindV1> {
    match engine {
        IvmEngine::C10 => C10Engine::fault_kind(error),
        IvmEngine::Baseline => BaselineEngine::fault_kind(error),
    }
}

/// Hash of the metering schedule of `engine`.
pub(crate) fn metering_schedule_hash(engine: IvmEngine) -> iroha_crypto::Hash {
    match engine {
        IvmEngine::C10 => C10Engine::metering_schedule_hash(),
        IvmEngine::Baseline => BaselineEngine::metering_schedule_hash(),
    }
}

impl IVM {
    /// Compiled identity of the profile the loaded image runs under.
    ///
    /// A VM without a loaded program reports the profile of the first-release interfaces.
    #[must_use]
    pub fn profile_id(&self) -> IvmProfileId {
        self.profile
    }

    /// Engine that executes the loaded image.
    #[must_use]
    pub fn engine(&self) -> IvmEngine {
        self.profile_id().engine()
    }

    /// Set the absolute heap ceiling of this instance and clamp the active limit to it.
    ///
    /// Hosts apply the governed heap limit through the facade so that the call does not depend
    /// on one engine's memory field.
    ///
    /// # Errors
    /// Refuses a ceiling below the allocated heap or above the ABI heap window.
    pub fn set_heap_max_limit(&mut self, limit: u64) -> Result<(), VMError> {
        self.memory.set_heap_max_limit(limit)
    }

    /// Length in bytes of the loaded code region.
    #[must_use]
    pub fn code_len(&self) -> u64 {
        self.memory.code_len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::encoding::wide::encode_halt;

    fn halting_program(policy: SyscallPolicy) -> Vec<u8> {
        let mut program = ProgramMetadata::default().encode_for_policy(policy);
        program.extend_from_slice(&encode_halt().to_le_bytes());
        program
    }

    fn baseline_halting_program() -> Vec<u8> {
        let mut program = ivm_abi::baseline::encode_header(
            &ProgramMetadata {
                max_cycles: 1_000,
                ..ProgramMetadata::default()
            },
            0,
        );
        program.extend_from_slice(&ivm_abi::baseline::encoding::encode_halt().to_le_bytes());
        program
    }

    #[test]
    fn facade_reports_the_engine_and_profile_of_the_loaded_image() {
        let mut vm = IVM::new(1_000);
        assert_eq!(
            vm.profile_id(),
            IvmProfileId::C10_ABI_V1,
            "an empty VM carries the explicit C10 profile of raw code"
        );
        assert_eq!(vm.engine(), IvmEngine::C10);
        vm.load_program(&halting_program(SyscallPolicy::AbiV1))
            .expect("load a generic C10 program");
        assert_eq!(vm.profile_id(), IvmProfileId::C10_ABI_V1);
        assert_eq!(vm.engine(), C10Engine::ID);
        assert_eq!(vm.code_len(), 4, "one halt instruction after the header");
        vm.run().expect("the dispatched engine runs the program");
    }

    #[test]
    fn preparation_dispatch_keeps_the_resolved_policy_and_never_retries_another() {
        let named = halting_program(SyscallPolicy::AbiV1RuntimeLifecycle);
        // A named-profile header under C10's own ABI V1 profile fails at that engine's header
        // check. The dispatcher reports that refusal and tries nothing else.
        let refusal = prepare(&named, None, IvmProfileId::C10_ABI_V1)
            .expect_err("the resolved profile does not match the header");
        assert!(
            matches!(
                refusal,
                VMError::ArtifactAbiHashMismatch { expected, actual }
                    if expected == IvmProfileId::C10_ABI_V1.abi_hash()
                        && actual
                            == IvmProfileId::of_policy(SyscallPolicy::AbiV1RuntimeLifecycle)
                                .abi_hash()
            ),
            "unexpected refusal: {refusal:?}"
        );
        assert_eq!(
            format!("{:?}", IVM::validate_program(&named).unwrap_err()),
            format!("{refusal:?}"),
            "the public C10 validator is the same dispatch"
        );
        prepare(
            &halting_program(SyscallPolicy::AbiV1),
            None,
            IvmProfileId::C10_ABI_V1,
        )
        .expect("the C10 profile admits its own header");
        // The interface policy runs the baseline engine, which reads its own layout and refuses
        // a C10 header without consulting C10.
        assert!(
            prepare(
                &halting_program(SyscallPolicy::AbiV1),
                None,
                IvmProfileId::of_policy(SyscallPolicy::AbiV1),
            )
            .is_err()
        );
        prepare(
            &baseline_halting_program(),
            None,
            IvmProfileId::of_policy(SyscallPolicy::AbiV1),
        )
        .expect("the interface policy admits a baseline header");
    }

    #[test]
    fn interface_policy_loads_the_baseline_engine() {
        let program = baseline_halting_program();
        // The C10 convenience loader reads the C10 layout and refuses a baseline header.
        assert!(IVM::validate_program(&program).is_err());
        let profile = IvmProfileId::of_policy(SyscallPolicy::AbiV1);
        assert_eq!(profile, IvmProfileId::BASELINE_ABI_V1);
        IVM::validate_program_for_policy(&program, SyscallPolicy::AbiV1)
            .expect("baseline admits its header");
        let mut vm = IVM::new(1_000);
        vm.load_program_for_policy(&program, SyscallPolicy::AbiV1)
            .expect("load a baseline program");
        assert_eq!(vm.profile_id(), profile);
        assert_eq!(vm.engine(), IvmEngine::Baseline);
        assert_eq!(vm.engine(), BaselineEngine::ID);
        vm.run().expect("the baseline engine runs HALT");
        assert_eq!(
            metering_schedule_hash(IvmEngine::Baseline),
            crate::gas::baseline::schedule_hash()
        );
        assert_eq!(
            fault_kind(IvmEngine::Baseline, &VMError::VectorExtensionDisabled),
            Some(IvmFaultKindV1::VectorExtensionDisabled)
        );
        // Loading a C10 artifact again restores the C10 engine.
        vm.load_program(&halting_program(SyscallPolicy::AbiV1))
            .expect("load a C10 program");
        assert_eq!(vm.engine(), IvmEngine::C10);
    }

    #[test]
    fn engine_projection_and_metering_identity_follow_the_engine() {
        for error in [VMError::OutOfGas, VMError::AssertionFailed] {
            assert_eq!(fault_kind(IvmEngine::Baseline, &error), error.fault_kind());
            assert_eq!(
                fault_kind(IvmEngine::C10, &error),
                fault_kind(IvmEngine::Baseline, &error)
            );
        }
        assert_eq!(fault_kind(IvmEngine::C10, &VMError::HostUnavailable), None);
        assert_eq!(
            metering_schedule_hash(IvmEngine::C10),
            crate::gas::c10_schedule_hash()
        );
        assert_eq!(
            crate::gas::schedule_hash_for(IvmEngine::C10),
            crate::gas::c10_schedule_hash()
        );
        assert_eq!(
            crate::gas::schedule_hash(),
            crate::gas::baseline::schedule_hash(),
            "the interface policy is priced by the baseline schedule"
        );
        let mut vm = IVM::new(0);
        vm.set_heap_max_limit(Memory::HEAP_MAX_SIZE)
            .expect("the ABI heap window is a valid ceiling");
        assert!(vm.set_heap_max_limit(Memory::HEAP_MAX_SIZE + 1).is_err());
    }
}
