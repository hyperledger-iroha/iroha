//! Private structural bridge from typed registered-platform statements to packed P-256 V3.
//!
//! The bridge accepts only the settled two-statement owner output, validates its
//! exact Eq-then-Ep geometry and identity, and retains that typed unverified
//! provenance beside two opaque circuit candidates. It confers no authority.

use core::{fmt, mem};

use super::{
    registered_platform_p256_statement::{
        validate_registered_platform_p256_source_pair_context_v2,
        UnverifiedRegisteredPlatformP256StatementSourcePairV2,
        UnverifiedRegisteredPlatformP256StatementV2,
        REGISTERED_PLATFORM_P256_CIRCUIT_SOURCE_AVAILABLE_V2,
        REGISTERED_PLATFORM_P256_PREHASH_OFFSET_V2, REGISTERED_PLATFORM_P256_SIGNATURE_OFFSET_V2,
        REGISTERED_PLATFORM_P256_SOURCE_PAIR_LOGICAL_BYTES_V2,
        REGISTERED_PLATFORM_P256_STATEMENT_BYTES_V2,
    },
    OfflineCashHalo2CircuitRoleV2, OfflineCashHalo2ParityV2,
};
use crate::zk::offline_cash_v1::{
    p256_packed_affine_ep_candidate_from_source_v3, p256_packed_affine_eq_candidate_from_source_v3,
    P256PackedAffineEpCircuitCandidateV3, P256PackedAffineEqCircuitCandidateV3,
    P256PackedStatementSourceV3,
};

/// The private source and opaque-candidate structural contract is implemented.
///
/// This is not an availability, readiness, verification, or release flag.
pub(super) const REGISTERED_PLATFORM_P256_CIRCUIT_SOURCE_STRUCTURAL_CONTRACT_IMPLEMENTED_V2: bool =
    true;
/// Stable logical bytes in the two opaque V3 statement-witness arrays.
pub(super) const REGISTERED_PLATFORM_P256_CIRCUIT_WITNESS_LOGICAL_BYTES_V2: usize = 322;
/// Stable logical bytes retained by the contextual source pair and two opaque candidates.
pub(super) const REGISTERED_PLATFORM_P256_CONTEXTUAL_CANDIDATES_LOGICAL_BYTES_V2: usize = 1_282;
/// Exact one-source temporary destination used while each opaque candidate is built.
pub(super) const REGISTERED_PLATFORM_P256_SOURCE_SCRATCH_LOGICAL_BYTES_V2: usize = 161;
/// Peak stable logical bytes for this source-only construction.
pub(super) const REGISTERED_PLATFORM_P256_SOURCE_PEAK_LOGICAL_BYTES_V2: usize = 1_443;
/// This private source tranche adds no transaction or session bytes.
pub(super) const REGISTERED_PLATFORM_P256_SOURCE_WIRE_DELTA_BYTES_V2: usize = 0;
/// This private source tranche adds no proof bytes.
pub(super) const REGISTERED_PLATFORM_P256_SOURCE_PROOF_DELTA_BYTES_V2: usize = 0;
/// This private source tranche adds no authenticated artifact bytes.
pub(super) const REGISTERED_PLATFORM_P256_SOURCE_ARTIFACT_DELTA_BYTES_V2: usize = 0;
/// This private source tranche allocates no Halo2 trace rows.
pub(super) const REGISTERED_PLATFORM_P256_SOURCE_TRACE_ROW_DELTA_V2: usize = 0;
/// This private source tranche adds no `Params` residence.
pub(super) const REGISTERED_PLATFORM_P256_SOURCE_PARAMS_DELTA_BYTES_V2: usize = 0;

const _: () = assert!(REGISTERED_PLATFORM_P256_CIRCUIT_SOURCE_STRUCTURAL_CONTRACT_IMPLEMENTED_V2);
const _: () = assert!(!REGISTERED_PLATFORM_P256_CIRCUIT_SOURCE_AVAILABLE_V2);
const _: () = assert!(REGISTERED_PLATFORM_P256_STATEMENT_BYTES_V2 == 161);
const _: () = assert!(REGISTERED_PLATFORM_P256_PREHASH_OFFSET_V2 == 65);
const _: () = assert!(REGISTERED_PLATFORM_P256_SIGNATURE_OFFSET_V2 == 97);
const _: () = assert!(
    REGISTERED_PLATFORM_P256_CIRCUIT_WITNESS_LOGICAL_BYTES_V2
        == 2 * REGISTERED_PLATFORM_P256_STATEMENT_BYTES_V2
);
const _: () = assert!(
    REGISTERED_PLATFORM_P256_CONTEXTUAL_CANDIDATES_LOGICAL_BYTES_V2
        == REGISTERED_PLATFORM_P256_SOURCE_PAIR_LOGICAL_BYTES_V2
            + REGISTERED_PLATFORM_P256_CIRCUIT_WITNESS_LOGICAL_BYTES_V2
);
const _: () = assert!(
    REGISTERED_PLATFORM_P256_SOURCE_SCRATCH_LOGICAL_BYTES_V2
        == REGISTERED_PLATFORM_P256_STATEMENT_BYTES_V2
);
const _: () = assert!(
    REGISTERED_PLATFORM_P256_SOURCE_PEAK_LOGICAL_BYTES_V2
        == REGISTERED_PLATFORM_P256_CONTEXTUAL_CANDIDATES_LOGICAL_BYTES_V2
            + REGISTERED_PLATFORM_P256_SOURCE_SCRATCH_LOGICAL_BYTES_V2
);
const _: () = assert!(REGISTERED_PLATFORM_P256_SOURCE_WIRE_DELTA_BYTES_V2 == 0);
const _: () = assert!(REGISTERED_PLATFORM_P256_SOURCE_PROOF_DELTA_BYTES_V2 == 0);
const _: () = assert!(REGISTERED_PLATFORM_P256_SOURCE_ARTIFACT_DELTA_BYTES_V2 == 0);
const _: () = assert!(REGISTERED_PLATFORM_P256_SOURCE_TRACE_ROW_DELTA_V2 == 0);
const _: () = assert!(REGISTERED_PLATFORM_P256_SOURCE_PARAMS_DELTA_BYTES_V2 == 0);

const SOURCE_ALREADY_POISONED: &str = "registered-platform P-256 statement source is poisoned";
#[cfg(test)]
const SOURCE_INJECTED_ERROR: &str = "injected registered-platform P-256 source error";

/// Structural rejection before either opaque circuit candidate is constructed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RegisteredPlatformP256CircuitSourceErrorV2 {
    EqParityMismatch,
    EpParityMismatch,
    EqRoleMismatch,
    EpRoleMismatch,
    StatementLengthMismatch,
    StatementBytesMismatch,
    MalformedTypedStatement,
    AuthenticatedContextMismatch,
    EqSourceRejected,
    EpSourceRejected,
}

impl fmt::Display for RegisteredPlatformP256CircuitSourceErrorV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::EqParityMismatch => "registered-platform P-256 Eq statement is out of order",
            Self::EpParityMismatch => "registered-platform P-256 Ep statement is out of order",
            Self::EqRoleMismatch => "registered-platform P-256 Eq statement has the wrong role",
            Self::EpRoleMismatch => "registered-platform P-256 Ep statement has the wrong role",
            Self::StatementLengthMismatch => {
                "registered-platform P-256 statement does not contain exactly 161 bytes"
            }
            Self::StatementBytesMismatch => {
                "registered-platform P-256 parity statements are not byte-identical"
            }
            Self::MalformedTypedStatement => {
                "registered-platform P-256 typed statement frame is malformed"
            }
            Self::AuthenticatedContextMismatch => {
                "registered-platform P-256 source pair does not match its authenticated current-helper context"
            }
            Self::EqSourceRejected => "registered-platform P-256 Eq source was rejected",
            Self::EpSourceRejected => "registered-platform P-256 Ep source was rejected",
        })
    }
}

impl std::error::Error for RegisteredPlatformP256CircuitSourceErrorV2 {}

enum RegisteredPlatformP256SourceStateV2<'a> {
    Ready(&'a UnverifiedRegisteredPlatformP256StatementV2),
    Poisoned,
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RegisteredPlatformP256SourceFaultV2 {
    None,
    Error,
    Panic,
}

struct RegisteredPlatformP256OneShotSourceV2<'a> {
    state: RegisteredPlatformP256SourceStateV2<'a>,
    #[cfg(test)]
    fault: RegisteredPlatformP256SourceFaultV2,
}

impl<'a> RegisteredPlatformP256OneShotSourceV2<'a> {
    fn from_validated(statement: &'a UnverifiedRegisteredPlatformP256StatementV2) -> Self {
        Self {
            state: RegisteredPlatformP256SourceStateV2::Ready(statement),
            #[cfg(test)]
            fault: RegisteredPlatformP256SourceFaultV2::None,
        }
    }

    fn read_once(
        &mut self,
        destination: &mut [u8; REGISTERED_PLATFORM_P256_STATEMENT_BYTES_V2],
    ) -> Result<(), &'static str> {
        destination.fill(0);
        let statement = match mem::replace(
            &mut self.state,
            RegisteredPlatformP256SourceStateV2::Poisoned,
        ) {
            RegisteredPlatformP256SourceStateV2::Ready(statement) => statement,
            RegisteredPlatformP256SourceStateV2::Poisoned => {
                return Err(SOURCE_ALREADY_POISONED);
            }
        };

        #[cfg(test)]
        match self.fault {
            RegisteredPlatformP256SourceFaultV2::None => {}
            RegisteredPlatformP256SourceFaultV2::Error => return Err(SOURCE_INJECTED_ERROR),
            RegisteredPlatformP256SourceFaultV2::Panic => {
                panic!("injected registered-platform P-256 source unwind")
            }
        }

        destination.copy_from_slice(statement.statement_bytes());
        Ok(())
    }

    #[cfg(test)]
    fn inject_fault_for_test(&mut self, fault: RegisteredPlatformP256SourceFaultV2) {
        self.fault = fault;
    }
}

struct RegisteredPlatformP256EqSourceV2<'a>(RegisteredPlatformP256OneShotSourceV2<'a>);

impl<'a> RegisteredPlatformP256EqSourceV2<'a> {
    fn from_validated(statement: &'a UnverifiedRegisteredPlatformP256StatementV2) -> Self {
        Self(RegisteredPlatformP256OneShotSourceV2::from_validated(
            statement,
        ))
    }
}

impl P256PackedStatementSourceV3 for RegisteredPlatformP256EqSourceV2<'_> {
    fn read_exact_statement(
        &mut self,
        destination: &mut [u8; REGISTERED_PLATFORM_P256_STATEMENT_BYTES_V2],
    ) -> Result<(), &'static str> {
        self.0.read_once(destination)
    }
}

struct RegisteredPlatformP256EpSourceV2<'a>(RegisteredPlatformP256OneShotSourceV2<'a>);

impl<'a> RegisteredPlatformP256EpSourceV2<'a> {
    fn from_validated(statement: &'a UnverifiedRegisteredPlatformP256StatementV2) -> Self {
        Self(RegisteredPlatformP256OneShotSourceV2::from_validated(
            statement,
        ))
    }
}

impl P256PackedStatementSourceV3 for RegisteredPlatformP256EpSourceV2<'_> {
    fn read_exact_statement(
        &mut self,
        destination: &mut [u8; REGISTERED_PLATFORM_P256_STATEMENT_BYTES_V2],
    ) -> Result<(), &'static str> {
        self.0.read_once(destination)
    }
}

/// Move-only opaque circuit pair retaining the exact typed unverified provenance.
#[must_use]
pub(super) struct UnverifiedRegisteredPlatformP256CircuitCandidatesV2 {
    source_pair: UnverifiedRegisteredPlatformP256StatementSourcePairV2,
    eq_fp: P256PackedAffineEqCircuitCandidateV3,
    ep_fq: P256PackedAffineEpCircuitCandidateV3,
}

impl UnverifiedRegisteredPlatformP256CircuitCandidatesV2 {
    pub(super) const fn source_pair(
        &self,
    ) -> &UnverifiedRegisteredPlatformP256StatementSourcePairV2 {
        &self.source_pair
    }

    pub(super) const fn provenance(&self) -> &[UnverifiedRegisteredPlatformP256StatementV2; 2] {
        self.source_pair.statements()
    }

    pub(super) const fn eq_fp(&self) -> &P256PackedAffineEqCircuitCandidateV3 {
        &self.eq_fp
    }

    pub(super) const fn ep_fq(&self) -> &P256PackedAffineEpCircuitCandidateV3 {
        &self.ep_fq
    }
}

/// Assemble opaque Eq/Fp and Ep/Fq candidates from the sole typed pair shape.
pub(super) fn assemble_unverified_registered_platform_p256_circuit_candidates_v2(
    source_pair: UnverifiedRegisteredPlatformP256StatementSourcePairV2,
) -> Result<
    UnverifiedRegisteredPlatformP256CircuitCandidatesV2,
    RegisteredPlatformP256CircuitSourceErrorV2,
> {
    validate_statement_pair(source_pair.statements())?;
    validate_registered_platform_p256_source_pair_context_v2(&source_pair)
        .map_err(|_| RegisteredPlatformP256CircuitSourceErrorV2::AuthenticatedContextMismatch)?;

    let statements = source_pair.statements();
    let eq_fp = p256_packed_affine_eq_candidate_from_source_v3(
        RegisteredPlatformP256EqSourceV2::from_validated(&statements[0]),
    )
    .map_err(|_| RegisteredPlatformP256CircuitSourceErrorV2::EqSourceRejected)?;
    let ep_fq = p256_packed_affine_ep_candidate_from_source_v3(
        RegisteredPlatformP256EpSourceV2::from_validated(&statements[1]),
    )
    .map_err(|_| RegisteredPlatformP256CircuitSourceErrorV2::EpSourceRejected)?;

    Ok(UnverifiedRegisteredPlatformP256CircuitCandidatesV2 {
        source_pair,
        eq_fp,
        ep_fq,
    })
}

fn validate_statement_pair(
    statements: &[UnverifiedRegisteredPlatformP256StatementV2; 2],
) -> Result<(), RegisteredPlatformP256CircuitSourceErrorV2> {
    let [eq, ep] = statements;
    if eq.parity() != OfflineCashHalo2ParityV2::Eq {
        return Err(RegisteredPlatformP256CircuitSourceErrorV2::EqParityMismatch);
    }
    if ep.parity() != OfflineCashHalo2ParityV2::Ep {
        return Err(RegisteredPlatformP256CircuitSourceErrorV2::EpParityMismatch);
    }
    if eq.role() != OfflineCashHalo2CircuitRoleV2::P256Signature {
        return Err(RegisteredPlatformP256CircuitSourceErrorV2::EqRoleMismatch);
    }
    if ep.role() != OfflineCashHalo2CircuitRoleV2::P256Signature {
        return Err(RegisteredPlatformP256CircuitSourceErrorV2::EpRoleMismatch);
    }
    if eq.statement_bytes().len() != REGISTERED_PLATFORM_P256_STATEMENT_BYTES_V2
        || ep.statement_bytes().len() != REGISTERED_PLATFORM_P256_STATEMENT_BYTES_V2
    {
        return Err(RegisteredPlatformP256CircuitSourceErrorV2::StatementLengthMismatch);
    }
    if eq.statement_bytes() != ep.statement_bytes() {
        return Err(RegisteredPlatformP256CircuitSourceErrorV2::StatementBytesMismatch);
    }

    let statement = eq.statement_bytes();
    if statement[0] != 4
        || statement[1..REGISTERED_PLATFORM_P256_PREHASH_OFFSET_V2]
            .iter()
            .all(|byte| *byte == 0)
        || statement[REGISTERED_PLATFORM_P256_PREHASH_OFFSET_V2
            ..REGISTERED_PLATFORM_P256_SIGNATURE_OFFSET_V2]
            .iter()
            .all(|byte| *byte == 0)
        || statement[REGISTERED_PLATFORM_P256_SIGNATURE_OFFSET_V2..129]
            .iter()
            .all(|byte| *byte == 0)
        || statement[129..].iter().all(|byte| *byte == 0)
    {
        return Err(RegisteredPlatformP256CircuitSourceErrorV2::MalformedTypedStatement);
    }
    Ok(())
}

#[cfg(test)]
#[path = "registered_platform_p256_circuit_source_tests.rs"]
mod tests;
