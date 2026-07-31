//! Structures, traits and impls related to *runtime* `Executor`s.

use std::{collections::BTreeSet, format, string::String, vec::Vec};

use iroha_data_model_derive::model;
use iroha_primitives::json::Json;
use iroha_schema::Ident;

#[cfg(any(feature = "transparent_api", feature = "ffi_import"))]
pub use self::model::*;
#[cfg(not(any(feature = "transparent_api", feature = "ffi_import")))]
pub use self::model::{
    ArtifactAbiHashMismatchInfo, ContractRejection, DecodedCodeSizeLimitInfo,
    DecodedInstructionLimitInfo, Executor, ExecutorDataModel, IvmAdmissionError,
    ManifestAbiHashMismatchInfo, ManifestCodeHashMismatchInfo, MaxCyclesExceedsFuelInfo,
    MaxCyclesExceedsUpperBoundInfo, UnsupportedVersionInfo, ValidationFail,
    VectorLengthTooLargeInfo,
};
use crate::transaction::executable::IvmBytecode;

#[model]
mod model {
    use derive_more::Constructor;
    use getset::Getters;
    use iroha_crypto::Hash;
    use iroha_macro::FromVariant;
    use iroha_schema::IntoSchema;
    use norito::codec::{Decode, Encode};

    use super::*;
    use crate::{isi, nexus::AxtRejectContext, parameter::CustomParameters, query};

    /// executor that checks if an operation satisfies some conditions.
    #[derive(
        Debug,
        Clone,
        PartialEq,
        Eq,
        PartialOrd,
        Ord,
        Constructor,
        Getters,
        Decode,
        Encode,
        IntoSchema,
    )]
    #[allow(clippy::multiple_inherent_impl)]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type(unsafe {robust}))]
    #[repr(transparent)]
    #[getset(get = "pub")]
    pub struct Executor {
        /// IVM bytecode of the executor
        pub bytecode: IvmBytecode,
    }

    /// Executor data model.
    ///
    /// Defined from within the executor, it describes certain structures the executor
    /// works with.
    ///
    /// Executor can define:
    ///
    /// - Permission tokens (see [`crate::permission::Permission`])
    /// - Configuration parameters (see [`crate::parameter::Parameter`])
    #[derive(
        Default,
        Debug,
        Clone,
        PartialEq,
        Eq,
        PartialOrd,
        Ord,
        Constructor,
        Getters,
        Decode,
        Encode,
        IntoSchema,
        derive_more::Display,
    )]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
    #[display("{self:?}")]
    #[getset(get = "pub")]
    pub struct ExecutorDataModel {
        /// Corresponds to the [`crate::parameter::Parameter::Custom`].
        /// Holds the initial value of the parameter
        #[cfg_attr(
            feature = "json",
            norito(with = "crate::parameter::custom::json_helpers")
        )]
        pub parameters: CustomParameters,
        /// Corresponds to instruction identifiers stored in [`crate::isi::InstructionBox`].
        /// Any type that implements [`crate::isi::Instruction`] should be listed here.
        pub instructions: BTreeSet<Ident>,
        /// Ids of permission tokens supported by the executor.
        pub permissions: BTreeSet<Ident>,
        /// Schema of executor defined data types (instructions, parameters, permissions)
        pub schema: Json,
    }

    /// Operation validation failed.
    ///
    /// # Note
    ///
    /// Keep in mind that *Validation* is not the right term
    /// (because *Runtime Executor* actually does execution too) and other names
    /// (like *Verification* or *Execution*) are being discussed.
    #[derive(
        Debug,
        displaydoc::Display,
        Clone,
        PartialEq,
        Eq,
        PartialOrd,
        Ord,
        FromVariant,
        Decode,
        Encode,
        IntoSchema,
    )]
    #[ignore_extra_doc_attributes]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type(opaque))]
    #[derive(thiserror::Error)]
    pub enum ValidationFail {
        /// Operation is not permitted: {0}
        NotPermitted(
            #[skip_from]
            #[skip_try_from]
            String,
        ),
        /// IVM admission/static checks failed with a structured reason.
        IvmAdmission(IvmAdmissionError),
        /// Instruction execution failed
        InstructionFailed(#[source] isi::error::InstructionExecutionError),
        /// Contract rejected the operation: {0}
        ContractRejected(ContractRejection),
        /// Query execution failed
        QueryFailed(#[source] query::error::QueryExecutionFail),
        /// Atomic cross-transaction policy rejected the request.
        AxtReject(AxtRejectContext),
        /// Operation is too complex; consider increasing the IVM runtime configuration parameters
        ///
        /// For example it's a very big IVM bytecode.
        ///
        /// It's different from [`crate::transaction::error::TransactionRejectionReason::LimitCheck`] because it depends on
        /// executor.
        TooComplex,
        /// Internal error occurred, please contact the support or check the logs if you are the node owner: {0}
        ///
        /// Usually means a bug inside **Runtime Executor** or **Iroha** implementation.
        InternalError(
            /// Contained error message if its used internally. Empty for external users.
            /// Never serialized to not to expose internal errors to the end user.
            #[codec(skip)]
            #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::secret_string"))]
            #[skip_from]
            #[skip_try_from]
            String,
        ),
    }

    /// Manifest-authenticated application error returned by a contract.
    #[derive(
        Debug,
        derive_more::Display,
        Clone,
        PartialEq,
        Eq,
        PartialOrd,
        Ord,
        Decode,
        Encode,
        IntoSchema,
    )]
    #[display("Contract {contract} rejected with {namespace}::{name} ({code})")]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type(opaque))]
    pub struct ContractRejection {
        /// Canonical source-level contract identity embedded in the artifact.
        pub contract: String,
        /// Declared error enum namespace.
        pub namespace: String,
        /// Declared variant name.
        pub name: String,
        /// Explicit stable non-zero application error code.
        pub code: u32,
    }

    /// Structured reasons for IVM admission/static validation failure.
    #[derive(
        Debug,
        displaydoc::Display,
        Clone,
        PartialEq,
        Eq,
        PartialOrd,
        Ord,
        Decode,
        Encode,
        IntoSchema,
    )]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize),
        norito(tag = "kind", content = "content")
    )]
    #[repr(u32)]
    pub enum IvmAdmissionError {
        /// IVM bytecode omits a non-zero `max_cycles` header field
        #[codec(index = 0)]
        MissingMaxCycles,
        /// Unsupported IVM version
        #[codec(index = 1)]
        UnsupportedVersion(UnsupportedVersionInfo),
        /// Unsupported IVM feature bits
        #[codec(index = 2)]
        UnsupportedFeatureBits(u8),
        /// Unsupported IVM ABI version
        #[codec(index = 3)]
        UnsupportedAbiVersion(u8),
        /// Program targets an ABI version that is not active under runtime governance
        #[codec(index = 4)]
        AbiVersionNotActive(u8),
        /// Vector length too large
        #[codec(index = 5)]
        VectorLengthTooLarge(VectorLengthTooLargeInfo),
        /// `max_cycles` exceeds upper bound
        #[codec(index = 6)]
        MaxCyclesExceedsUpperBound(MaxCyclesExceedsUpperBoundInfo),
        /// `max_cycles` exceeds configured fuel limit
        #[codec(index = 7)]
        MaxCyclesExceedsFuel(MaxCyclesExceedsFuelInfo),
        /// Decoded instruction count exceeds admission limit
        #[codec(index = 8)]
        DecodedInstructionCountExceeded(DecodedInstructionLimitInfo),
        /// Decoded byte length exceeds admission limit
        #[codec(index = 9)]
        DecodedCodeSizeExceeded(DecodedCodeSizeLimitInfo),
        /// Kotodama bytecode decode failed: {0}
        #[codec(index = 10)]
        BytecodeDecodingFailed(String),
        /// Manifest `code_hash` mismatch
        #[codec(index = 11)]
        ManifestCodeHashMismatch(ManifestCodeHashMismatchInfo),
        /// Manifest `abi_hash` mismatch with node policy
        #[codec(index = 12)]
        ManifestAbiHashMismatch(ManifestAbiHashMismatchInfo),
        /// Contract manifest omits the required `code_hash`
        #[codec(index = 13)]
        ManifestCodeHashMissing,
        /// Contract manifest omits the required `abi_hash`
        #[codec(index = 14)]
        ManifestAbiHashMissing,
        /// Transaction `contract_manifest` metadata is malformed
        #[codec(index = 15)]
        ManifestMalformed,
        /// Artifact-carried ABI hash does not match the runtime descriptor
        #[codec(index = 16)]
        ArtifactAbiHashMismatch(ArtifactAbiHashMismatchInfo),
        /// Generic IVM program invokes a contract-bound syscall
        #[codec(index = 17)]
        GenericSyscallNotAllowed(u32),
    }

    /// Unsupported IVM version details
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct UnsupportedVersionInfo {
        /// Major version
        pub major: u8,
        /// Minor version
        pub minor: u8,
    }

    /// Vector length limit violation details
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct VectorLengthTooLargeInfo {
        /// Provided vector length
        pub vector_length: u8,
        /// Maximum allowed
        pub max_allowed: u8,
    }

    /// Max cycles limit violation details
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct MaxCyclesExceedsUpperBoundInfo {
        /// Header `max_cycles` value
        pub max_cycles: u64,
        /// Upper bound
        pub upper_bound: u64,
    }

    /// Fuel limit violation details
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct MaxCyclesExceedsFuelInfo {
        /// Header `max_cycles` value
        pub max_cycles: u64,
        /// Configured fuel limit
        pub fuel_limit: u64,
    }

    /// Decoded instruction count violation details
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct DecodedInstructionLimitInfo {
        /// Number of decoded instructions observed during admission
        pub decoded_instructions: u64,
        /// Configured limit
        pub limit: u64,
    }

    /// Decoded byte length violation details
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct DecodedCodeSizeLimitInfo {
        /// Total decoded byte length observed during admission
        pub decoded_bytes: u64,
        /// Configured limit in bytes
        pub limit: u64,
    }

    /// Manifest code hash mismatch (expected, actual)
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct ManifestCodeHashMismatchInfo {
        /// Expected `code_hash` (from manifest)
        pub expected: Hash,
        /// Actual `code_hash` (computed)
        pub actual: Hash,
    }

    /// Manifest abi hash mismatch (expected, actual)
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct ManifestAbiHashMismatchInfo {
        /// Expected `abi_hash` (from manifest)
        pub expected: Hash,
        /// Actual `abi_hash` (computed for policy)
        pub actual: Hash,
    }

    /// Artifact ABI hash mismatch (runtime descriptor, authenticated artifact binding).
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct ArtifactAbiHashMismatchInfo {
        /// ABI hash required by the executing runtime.
        pub expected: Hash,
        /// ABI hash authenticated by the submitted artifact's CNTR section.
        pub actual: Hash,
    }

    // Client builds that disable `transparent_api`/`ffi_import` skip re-exporting
    // these detail structs (see conditional `pub use` above) while still allowing
    // servers and FFI consumers to access the full metadata when needed.
}

// Unify conversions from query `FindError` into `ValidationFail` by wrapping it in
// `QueryExecutionFail::Find`. This enables ergonomic `?` usage at call sites
// that return `ValidationFail` while invoking helpers that may produce `FindError`.
impl From<crate::query::error::FindError> for ValidationFail {
    fn from(e: crate::query::error::FindError) -> Self {
        ValidationFail::QueryFailed(crate::query::error::QueryExecutionFail::Find(e))
    }
}

#[cfg(feature = "json")]
impl norito::json::FastJsonWrite for Executor {
    fn write_json(&self, out: &mut String) {
        norito::json::JsonSerialize::json_serialize(&self.bytecode, out);
    }
}

#[cfg(feature = "json")]
impl norito::json::JsonDeserialize for Executor {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        let bytecode = <IvmBytecode as norito::json::JsonDeserialize>::json_deserialize(parser)?;
        Ok(Self { bytecode })
    }
}

/// Result type that every executor should return.
pub type Result<T = (), E = crate::ValidationFail> = core::result::Result<T, E>;

pub mod prelude {
    //! The prelude re-exports most commonly used traits, structs and macros from this crate.
    pub use super::{Executor, ExecutorDataModel};
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transaction::executable::IvmBytecode;
    use norito::codec::{DecodeAll, Encode};

    #[test]
    fn bytecode_getter_returns_inner_bytecode() {
        let code = IvmBytecode::from_compiled(vec![1, 2, 3]);
        let executor = Executor::new(code.clone());
        assert_eq!(executor.bytecode().as_ref(), code.as_ref());
    }

    #[test]
    fn ivm_admission_error_wire_tags_are_pinned_and_roundtrip() {
        let hash = iroha_crypto::Hash::new(b"ivm-admission-error-tag-test");
        let cases = [
            (IvmAdmissionError::MissingMaxCycles, 0),
            (
                IvmAdmissionError::UnsupportedVersion(UnsupportedVersionInfo {
                    major: 2,
                    minor: 3,
                }),
                1,
            ),
            (IvmAdmissionError::UnsupportedFeatureBits(0x80), 2),
            (IvmAdmissionError::UnsupportedAbiVersion(2), 3),
            (IvmAdmissionError::AbiVersionNotActive(2), 4),
            (
                IvmAdmissionError::VectorLengthTooLarge(VectorLengthTooLargeInfo {
                    vector_length: 65,
                    max_allowed: 64,
                }),
                5,
            ),
            (
                IvmAdmissionError::MaxCyclesExceedsUpperBound(MaxCyclesExceedsUpperBoundInfo {
                    max_cycles: 2,
                    upper_bound: 1,
                }),
                6,
            ),
            (
                IvmAdmissionError::MaxCyclesExceedsFuel(MaxCyclesExceedsFuelInfo {
                    max_cycles: 2,
                    fuel_limit: 1,
                }),
                7,
            ),
            (
                IvmAdmissionError::DecodedInstructionCountExceeded(DecodedInstructionLimitInfo {
                    decoded_instructions: 2,
                    limit: 1,
                }),
                8,
            ),
            (
                IvmAdmissionError::DecodedCodeSizeExceeded(DecodedCodeSizeLimitInfo {
                    decoded_bytes: 2,
                    limit: 1,
                }),
                9,
            ),
            (IvmAdmissionError::BytecodeDecodingFailed("bad".into()), 10),
            (
                IvmAdmissionError::ManifestCodeHashMismatch(ManifestCodeHashMismatchInfo {
                    expected: hash,
                    actual: hash,
                }),
                11,
            ),
            (
                IvmAdmissionError::ManifestAbiHashMismatch(ManifestAbiHashMismatchInfo {
                    expected: hash,
                    actual: hash,
                }),
                12,
            ),
            (IvmAdmissionError::ManifestCodeHashMissing, 13),
            (IvmAdmissionError::ManifestAbiHashMissing, 14),
            (IvmAdmissionError::ManifestMalformed, 15),
            (
                IvmAdmissionError::ArtifactAbiHashMismatch(ArtifactAbiHashMismatchInfo {
                    expected: hash,
                    actual: hash,
                }),
                16,
            ),
            (IvmAdmissionError::GenericSyscallNotAllowed(0x47), 17),
        ];

        for (error, expected_tag) in cases {
            let encoded = error.encode();
            assert!(encoded.len() >= 4);
            assert_eq!(
                u32::from_le_bytes(encoded[..4].try_into().expect("four-byte enum tag")),
                expected_tag
            );
            assert_eq!(
                IvmAdmissionError::decode_all(&mut encoded.as_slice())
                    .expect("decode admission error"),
                error
            );
        }

        let unknown = 18_u32.encode();
        assert!(
            IvmAdmissionError::decode_all(&mut unknown.as_slice()).is_err(),
            "unknown admission error tags must fail closed"
        );
    }
}
