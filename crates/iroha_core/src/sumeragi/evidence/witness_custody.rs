//! Original preparation-pool custody for native proof result-witness bytes.
use super::*;
use iroha_allocation::PrepaidSharedError;
use iroha_sumeragi::message::ByteAdmissionError;

fn preparation(error: ByteAdmissionError) -> EvidencePreparationError {
    match error {
        ByteAdmissionError::Buffer(error) => error.into(),
        ByteAdmissionError::ControlAdmission(error) => EvidencePreparationError::Admission(error),
        ByteAdmissionError::ControlAllocation(PrepaidSharedError::Allocator {
            requested_bytes,
        }) => EvidencePreparationError::Allocator { requested_bytes },
        // A fresh decoded proof has neither foreign custody nor invalid witness geometry.
        // Insufficient prepaid control credit likewise contradicts its constructor contract.
        ByteAdmissionError::Length { .. }
        | ByteAdmissionError::ForeignBudget
        | ByteAdmissionError::ControlAllocation(PrepaidSharedError::Reservation(_)) => {
            EvidencePreparationError::Invariant
        }
    }
}

pub(super) fn decode(
    proof: &Evidence,
    budget: &AllocationBudget,
) -> Result<NativeEvidence, EvidenceAdmissionError> {
    let mut native = proof
        .decode_native()
        .map_err(EvidenceAdmissionError::from)?;
    if !cfg!(all(test, sumeragi_core_mutation = "HC9")) {
        native.admit_result_witnesses(budget).map_err(preparation)?;
        if !native.result_witnesses_admitted_to(budget) {
            return Err(EvidencePreparationError::Invariant.into());
        }
    }
    // TODO(S8): initial decoding, canonical pair scratch and the remaining Vec/Box graph
    // still need original-pool ownership. Only witness backing/control are admitted here.
    // On capture failure this decoded proof drops/refunds; no decoder progress is retained.
    Ok(native)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn witness_refusal_mapping_preserves_allocator_size_and_terminal_invariants() {
        for error in [
            ByteAdmissionError::Buffer(iroha_allocation::ChargedBufferError::Allocator {
                requested_bytes: 123,
            }),
            ByteAdmissionError::ControlAllocation(PrepaidSharedError::Allocator {
                requested_bytes: 123,
            }),
        ] {
            assert_eq!(
                preparation(error),
                EvidencePreparationError::Allocator {
                    requested_bytes: 123
                }
            );
        }
        for error in [
            ByteAdmissionError::Length { length: 0 },
            ByteAdmissionError::ForeignBudget,
        ] {
            let error = EvidenceAdmissionError::Preparation(preparation(error));
            assert!(!super::super::admission::retryable(&error));
            assert!(matches!(
                error,
                EvidenceAdmissionError::Preparation(EvidencePreparationError::Invariant)
            ));
        }
        let pool = AllocationBudget::new(0);
        let buffer = ChargedBuffer::<u8>::new(1, &pool)
            .err()
            .expect("zero original capacity");
        let failure = preparation(ByteAdmissionError::Buffer(buffer));
        assert!(matches!(
            failure,
            EvidencePreparationError::Admission(
                iroha_allocation::AllocationRefusal::ExceedsLimit {
                    requested_bytes: 1,
                    ..
                }
            )
        ));
        assert!(super::super::admission::retryable(&failure.into()));
    }
}
