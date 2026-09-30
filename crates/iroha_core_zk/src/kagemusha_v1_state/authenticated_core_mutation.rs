//! Original typed mutation material for canonical pending checkpoint reconstruction.
//! Decoding this enum grants no authority. Only a concrete native owner can revalidate it.

use super::*;
use iroha_data_model::kagemusha::{KagemushaCommitCertificateV1, KagemushaRedemptionProofV1};

#[derive(Clone, norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core::zk::kagemusha_v1_state::AuthenticatedCoreMutationOriginalV1")]
pub(super) enum Mutation {
    JournalHeads,
    PrepareSend {
        operation_id: DigestV1,
        preparation: SendSplitPreparationV1,
    },
    PrepareRedemption {
        operation_id: DigestV1,
        preparation: RedeemSplitPreparationV1,
    },
    CandidateProof {
        operation_id: DigestV1,
        proof: KagemushaPairedProofV1,
    },
    Commit {
        operation_id: DigestV1,
        certificate: KagemushaCommitCertificateV1,
        device_original: KagemushaOriginalOutgoingHardwareCommitV1,
        // Raw evidence is reverified on reconstruction. Opaque history capabilities
        // never enter a serialized record and cannot be recreated by decoding one.
        hardware_certificate: HardwareTransitionCertificateV1,
        proof: KagemushaPairedProofV1,
        reference_ms: u64,
    },
    FinalPayment {
        payment: KagemushaPaymentV1,
        retry_metadata: Vec<u8>,
    },
    FinalRedemption {
        proof: KagemushaRedemptionProofV1,
        retry_metadata: Vec<u8>,
    },
    // Closed native incoming archive. Only the incoming module's bounded exact canonical
    // decoder and original proof/Guard/history verifier can interpret this material.
    Incoming {
        canonical_original: Vec<u8>,
    },
}
