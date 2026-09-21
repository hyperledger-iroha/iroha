//! Pure instruction ownership checks shared by native signer facades and their transports.

use super::SorafsNativeTransactionSignerRoleV1;
use iroha_data_model::{
    isi::sorafs::{
        AdvanceSorafsReserveLifecycle, ApplySorafsRepairTaskAction, ChargeSorafsReserveRent,
        DecideSorafsReserveAppeal, DecideSorafsReserveMovement, DrawSorafsReserveCredit,
        MaintainSorafsOrderbook, MatchSorafsOrderbook, RecordSorafsOrderbookSettlementReceipt,
        RegisterSorafsReserveAccount, RepaySorafsReserveCredit, RequestSorafsReserveMovement,
        SubmitSorafsProofOutcome, SubmitSorafsRepairAppeal, SubmitSorafsRepairTask,
        SubmitSorafsReserveAppeal,
    },
    transaction::{Executable, TransactionPayload},
};

/// Whether the payload contains exactly one direct instruction owned by this native signer role.
///
/// This pure check performs no provider calls, decoding, recursive instruction traversal or state
/// reads. Empty/multiple instructions, wrappers, mixed batches, contract calls and IVM executions
/// are rejected even when they contain an otherwise allowed instruction. Proof attachments are
/// also rejected, matching the qualified facade's sidecar-free signed-output contract. Native
/// forwarders own field-level state/permission eligibility; this predicate does not establish that eligibility,
/// network identity, account binding, fee approval or hardware custody.
#[must_use]
pub fn sorafs_native_transaction_payload_matches_role_v1(
    role: SorafsNativeTransactionSignerRoleV1,
    payload: &TransactionPayload,
) -> bool {
    if payload.attachments.is_some() {
        return false;
    }
    let Executable::Instructions(instructions) = payload.instructions() else {
        return false;
    };
    let [instruction] = instructions.as_ref() else {
        return false;
    };
    let instruction = instruction.as_any();
    match role {
        SorafsNativeTransactionSignerRoleV1::ProofOutcome => {
            instruction.is::<SubmitSorafsProofOutcome>()
        }
        SorafsNativeTransactionSignerRoleV1::Repair => {
            instruction.is::<SubmitSorafsRepairTask>()
                || instruction.is::<ApplySorafsRepairTaskAction>()
                || instruction.is::<SubmitSorafsRepairAppeal>()
        }
        SorafsNativeTransactionSignerRoleV1::Reserve => {
            instruction.is::<RegisterSorafsReserveAccount>()
                || instruction.is::<RequestSorafsReserveMovement>()
                || instruction.is::<DecideSorafsReserveMovement>()
                || instruction.is::<ChargeSorafsReserveRent>()
                || instruction.is::<AdvanceSorafsReserveLifecycle>()
                || instruction.is::<DrawSorafsReserveCredit>()
                || instruction.is::<RepaySorafsReserveCredit>()
                || instruction.is::<SubmitSorafsReserveAppeal>()
                || instruction.is::<DecideSorafsReserveAppeal>()
        }
        SorafsNativeTransactionSignerRoleV1::Orderbook => {
            instruction.is::<MatchSorafsOrderbook>()
                || instruction.is::<MaintainSorafsOrderbook>()
                || instruction.is::<RecordSorafsOrderbookSettlementReceipt>()
        }
    }
}
