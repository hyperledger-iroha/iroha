//! Completed incoming-proof verdicts stay separate from original local verifier failures.

use iroha_pasta::msm::MsmError;
use iroha_plonk::{
    VerifyError,
    pcs::{ipa::IpaError, multiopen::MultiopenError},
    protocol::ProtocolError,
    transcript::TranscriptError,
};

use super::Error;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Attempt {
    Rejected,
    Cancelled,
    MemoryBudget,
    Local,
}

fn kernel(error: &MsmError) -> Attempt {
    match error {
        MsmError::Cancelled => Attempt::Cancelled,
        MsmError::Budget(_) => Attempt::MemoryBudget,
        MsmError::LengthMismatch(_) | MsmError::ScalarBound | MsmError::TooLarge { .. } => {
            Attempt::Local
        }
    }
}

fn transcript(error: &TranscriptError) -> Attempt {
    match error {
        TranscriptError::ProfileMismatch => Attempt::Local,
        TranscriptError::ProofTruncated
        | TranscriptError::TrailingBytes { .. }
        | TranscriptError::NonCanonicalScalar
        | TranscriptError::InvalidPoint
        | TranscriptError::IdentityPoint => Attempt::Rejected,
    }
}

fn ipa(error: &IpaError) -> Attempt {
    match error {
        IpaError::Cancelled => Attempt::Cancelled,
        IpaError::Msm(original) => kernel(original),
        IpaError::Transcript(original) => transcript(original),
        IpaError::OpeningFailed
        | IpaError::FoldedGeneratorMismatch
        | IpaError::ZeroChallenge { .. } => Attempt::Rejected,
        IpaError::LengthMismatch { .. } | IpaError::ParamsTooSmall { .. } => Attempt::Local,
    }
}

fn multiopen(error: &MultiopenError) -> Attempt {
    match error {
        MultiopenError::Cancelled => Attempt::Cancelled,
        MultiopenError::Ipa(original) => ipa(original),
        MultiopenError::ConflictingEvaluations { .. }
        | MultiopenError::PointCollision
        | MultiopenError::DegenerateChallenge => Attempt::Rejected,
        MultiopenError::NoQueries
        | MultiopenError::TooManyQueries
        | MultiopenError::Shape { .. }
        | MultiopenError::UnsupportedDomain { .. } => Attempt::Local,
    }
}

// Trusted parameter/table construction cannot reject incoming material, even when a
// nested error happens to be named like an equation or transcript rejection.
fn context(attempt: Attempt) -> Attempt {
    match attempt {
        Attempt::Rejected => Attempt::Local,
        other => other,
    }
}

fn verification(error: &VerifyError) -> Attempt {
    match error {
        VerifyError::Cancelled => Attempt::Cancelled,
        VerifyError::Descriptor(original) if original.is_cancelled() => Attempt::Cancelled,
        VerifyError::VerifyingKey(original) if original.is_cancelled() => Attempt::Cancelled,
        VerifyError::Ipa(original) => ipa(original),
        VerifyError::Multiopen(original) => multiopen(original),
        VerifyError::Protocol(ProtocolError::Plan(original)) => context(multiopen(original)),
        VerifyError::BatchItem { error, .. } => verification(error),
        VerifyError::Transcript(original) => transcript(original),
        VerifyError::ProofLength { .. }
        | VerifyError::DegenerateChallenge
        | VerifyError::BatchRejected => Attempt::Rejected,
        VerifyError::Descriptor(_)
        | VerifyError::VerifyingKey(_)
        | VerifyError::KeyMismatch
        | VerifyError::ParamsMismatch
        | VerifyError::InstanceType { .. }
        | VerifyError::InstanceColumns { .. }
        | VerifyError::InstanceLength { .. }
        | VerifyError::IdentityInstanceCommitment { .. }
        | VerifyError::SuffixRequired
        | VerifyError::Protocol(_) => Attempt::Local,
    }
}

fn recursion(error: &iroha_plonk_recursion::Error) -> Attempt {
    use iroha_plonk_recursion::Error as E;
    match error {
        E::Cancelled => Attempt::Cancelled,
        E::Kernel(original) => kernel(original),
        E::Parameters(original) => context(ipa(original)),
        E::Encoding(original) => transcript(original),
        E::Length { .. }
        | E::SourceK
        | E::Padding
        | E::ZeroChallenge { .. }
        | E::Undecidable
        | E::FoldEquation => Attempt::Rejected,
        E::EmptyInputs
        | E::MissingFullLengthInput
        | E::Purpose
        | E::InputCount
        | E::NotCorrected => Attempt::Local,
    }
}

pub(super) fn verification_error(original: VerifyError) -> Error {
    match verification(&original) {
        Attempt::Cancelled => Error::Cancelled,
        Attempt::Rejected => Error::Proof,
        Attempt::MemoryBudget | Attempt::Local => Error::Verification(original),
    }
}

pub(super) fn recursion_error(original: iroha_plonk_recursion::Error) -> Error {
    match recursion(&original) {
        Attempt::Cancelled => Error::Cancelled,
        Attempt::Rejected => Error::Proof,
        Attempt::MemoryBudget | Attempt::Local => Error::Recursion(original),
    }
}

impl Error {
    /// Whether an original native kernel refused its explicit memory budget.
    /// This diagnostic does not report physical allocation failure or a proof verdict.
    #[must_use]
    pub fn is_verification_memory_budget(&self) -> bool {
        match self {
            Self::Verification(original) => verification(original) == Attempt::MemoryBudget,
            Self::Recursion(original) => recursion(original) == Attempt::MemoryBudget,
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests;
