//! Typed local-test rejection expectations shared by compiler and in-process runner.
// Norito derives probe schema-structural; this module does not enable structural schema emission.
#![allow(unexpected_cfgs)]
use iroha_data_model::smart_contract::manifest::ContractErrorTypeDescriptor;
use ivm_abi::error::VmTrapKind;

/// Exact reason requested by a test-only nested invocation.
#[derive(Clone, Debug, PartialEq, Eq, norito::Encode, norito::Decode)]
pub enum RejectionExpectation {
    /// Explicitly accept any rejection through `test::expect_any_reject_as`.
    Any,
    /// Invocation did not have the required entrypoint permission.
    PermissionDenied,
    /// Input could not be encoded against the declared argument schema.
    InvalidArguments,
    /// Runtime emitted the specified nominal contract error and variant.
    Contract {
        /// Complete expected nominal error schema.
        descriptor: ContractErrorTypeDescriptor,
        /// Exact enum-local variant code.
        code: u32,
    },
    /// Runtime reached the specifically selected VM trap category.
    Trap(RejectionTrap),
}

macro_rules! rejection_traps {
    ($($name:ident => $trap:ident),+ $(,)?) => {
        /// Compiler-owned runtime-trap selectors; contract errors require nominal variants.
        #[derive(Clone, Copy, Debug, PartialEq, Eq, norito::Encode, norito::Decode)]
        pub enum RejectionTrap {
            $(#[doc = concat!("Require the `", stringify!($trap), "` runtime trap.")]
            $name,)+
        }
        impl RejectionTrap {
            /// Exact VM category selected by this constant.
            pub const fn trap_kind(self) -> VmTrapKind {
                match self { $(Self::$name => VmTrapKind::$trap,)+ }
            }
            /// Canonical source spelling of this test-only selector.
            pub const fn source_name(self) -> &'static str {
                match self { $(Self::$name => concat!("test::Rejection::", stringify!($name)),)+ }
            }
        }
        /// Every accepted exact test selector, including pre-runtime rejection stages.
        pub const REJECTION_SELECTORS: &[&str] = &[
            "test::Rejection::PermissionDenied", "test::Rejection::InvalidArguments",
            $(concat!("test::Rejection::", stringify!($name)),)+
        ];
        impl RejectionExpectation {
            /// Resolve a compiler-owned stage or runtime-trap literal.
            pub fn from_selector(name: &str) -> Option<Self> {
                Some(match name {
                    "test::Rejection::PermissionDenied" => Self::PermissionDenied,
                    "test::Rejection::InvalidArguments" => Self::InvalidArguments,
                    $(concat!("test::Rejection::", stringify!($name)) => Self::Trap(RejectionTrap::$name),)+
                    _ => return None,
                })
            }
        }
    };
}
rejection_traps! {
    OutOfGas => OutOfGas, OutOfMemory => OutOfMemory, MemoryFault => MemoryFault,
    DecodeError => DecodeError, InvalidOpcode => InvalidOpcode, UnknownSyscall => UnknownSyscall,
    NotImplemented => NotImplemented, SyscallGasQuoteExceeded => SyscallGasQuoteExceeded,
    SyscallMeteringModeMismatch => SyscallMeteringModeMismatch, GasCostOverflow => GasCostOverflow,
    NumericFault => NumericFault, PointerAbiFault => PointerAbiFault,
    AssertionFailed => AssertionFailed, ExceededMaxCycles => ExceededMaxCycles,
    InvalidMetadata => InvalidMetadata, UnsupportedProgramVersion => UnsupportedProgramVersion,
    UnsupportedProgramFeatureBits => UnsupportedProgramFeatureBits,
    UnsupportedProgramAbiVersion => UnsupportedProgramAbiVersion,
    ProgramVectorLengthTooLarge => ProgramVectorLengthTooLarge,
    ArtifactAbiHashMismatch => ArtifactAbiHashMismatch,
    GenericSyscallNotAllowed => GenericSyscallNotAllowed, InvalidVectorLength => InvalidVectorLength,
    MissingHalt => MissingHalt, RuntimePermissionDenied => PermissionDenied,
    PrivacyViolation => PrivacyViolation, RegisterOutOfBounds => RegisterOutOfBounds,
    NoritoInvalid => NoritoInvalid, AbiTypeNotAllowed => AbiTypeNotAllowed,
    HostOutputBudgetExceeded => HostOutputBudgetExceeded, AmxBudgetExceeded => AmxBudgetExceeded,
}
impl RejectionExpectation {
    /// Compare an observed runtime failure without matching human-readable error text.
    pub fn matches_runtime(&self, error: &ivm_abi::VMError, trap: Option<VmTrapKind>) -> bool {
        match self {
            Self::Any => true,
            Self::Contract { descriptor, code } => matches!(error.as_unmetered(),
                ivm_abi::VMError::ContractAbort { error_type, schema_hash, code: actual, .. }
                    if error_type == &descriptor.identity && schema_hash == &descriptor.schema_hash() && actual == code),
            Self::Trap(expected) => trap == Some(expected.trap_kind()),
            Self::PermissionDenied | Self::InvalidArguments => false,
        }
    }
    /// Validate decoded expectations before running any nested contract.
    pub fn validate(&self) -> bool {
        match self {
            Self::Contract { descriptor, code } => {
                descriptor.validate() && descriptor.variant(*code).is_some()
            }
            _ => true,
        }
    }
    /// Human-readable expected reason, preserving its nominal namespace.
    pub fn description(&self) -> String {
        match self {
            Self::Any => "any rejection (explicitly requested)".to_owned(),
            Self::PermissionDenied => "invocation PermissionDenied".to_owned(),
            Self::InvalidArguments => "argument-schema InvalidArguments".to_owned(),
            Self::Trap(trap) => trap.source_name().to_owned(),
            Self::Contract { descriptor, code } => format!(
                "{}::{} (code {code})",
                descriptor.identity,
                descriptor
                    .variant(*code)
                    .map_or("<invalid>", |variant| variant.name.as_str())
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn selector_inventory_is_exact_and_roundtrips_the_canonical_codec() {
        for name in REJECTION_SELECTORS {
            let expected = RejectionExpectation::from_selector(name).expect("registered selector");
            let bytes =
                ivm_abi::codec::encode_canonical_norito(&expected).expect("encode expectation");
            let decoded: RejectionExpectation =
                norito::decode_canonical(&bytes).expect("decode expectation");
            assert_eq!(expected, decoded);
            assert!(decoded.validate());
            assert!(!decoded.description().is_empty());
        }
        assert!(RejectionExpectation::from_selector("test::Rejection::ContractAbort").is_none());
        assert!(RejectionExpectation::from_selector("PermissionDenied").is_none());
    }
    #[test]
    fn nominal_expectations_validate_the_exact_variant_schema() {
        let descriptor = ivm_abi::error_types::list_error_type();
        let expected = RejectionExpectation::Contract {
            code: descriptor.variants[0].code,
            descriptor: descriptor.clone(),
        };
        assert!(expected.validate());
        assert!(expected.description().contains(&descriptor.identity));
        assert!(
            !RejectionExpectation::Contract {
                descriptor,
                code: 0
            }
            .validate()
        );
        assert!(RejectionExpectation::Any.validate());
    }
    #[test]
    fn matching_retains_nominal_identity_schema_and_stage_boundaries() {
        let descriptor = ivm_abi::error_types::list_error_type();
        let code = descriptor.variants[0].code;
        let expected = RejectionExpectation::Contract {
            descriptor: descriptor.clone(),
            code,
        };
        let error = ivm_abi::VMError::ContractAbort {
            contract: "test".to_owned(),
            name: "Failure".to_owned(),
            error_type: descriptor.identity.clone(),
            schema_hash: descriptor.schema_hash(),
            code,
        };
        assert!(expected.matches_runtime(&error, Some(VmTrapKind::ContractAbort)));
        assert!(expected.matches_runtime(
            &ivm_abi::VMError::Metered {
                gas: 1,
                source: Box::new(error.clone())
            },
            Some(VmTrapKind::ContractAbort)
        ));
        let wrong_type = ivm_abi::VMError::ContractAbort {
            contract: "test".to_owned(),
            name: "Failure".to_owned(),
            error_type: "Other".to_owned(),
            schema_hash: descriptor.schema_hash(),
            code,
        };
        let wrong_schema = ivm_abi::VMError::ContractAbort {
            contract: "test".to_owned(),
            name: "Failure".to_owned(),
            error_type: descriptor.identity.clone(),
            schema_hash: [0; 32],
            code,
        };
        let wrong_code = ivm_abi::VMError::ContractAbort {
            contract: "test".to_owned(),
            name: "Failure".to_owned(),
            error_type: descriptor.identity.clone(),
            schema_hash: descriptor.schema_hash(),
            code: code + 1,
        };
        for error in [
            wrong_type,
            wrong_schema,
            wrong_code,
            ivm_abi::VMError::OutOfGas,
        ] {
            assert!(!expected.matches_runtime(&error, Some(VmTrapKind::ContractAbort)));
        }
        assert!(!RejectionExpectation::PermissionDenied.matches_runtime(
            &ivm_abi::VMError::PermissionDenied,
            Some(VmTrapKind::PermissionDenied)
        ));
        assert!(
            RejectionExpectation::Trap(RejectionTrap::OutOfGas)
                .matches_runtime(&ivm_abi::VMError::OutOfGas, Some(VmTrapKind::OutOfGas))
        );
        assert!(RejectionExpectation::Any.matches_runtime(&ivm_abi::VMError::OutOfGas, None));
    }
}
