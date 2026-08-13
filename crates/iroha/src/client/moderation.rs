//! Route-to-instruction validation for native `SoraFS` moderation submissions.
use eyre::{Result, eyre};
use iroha_data_model::{
    isi::sorafs::{
        AcceptSorafsModerationJurorAssignment, ActivateSorafsModerationCase,
        FinalizeSorafsModerationCase, FinalizeSorafsModerationSortition,
        RaiseSorafsModerationChallenge, RegisterSorafsModerationJurorEligibility,
        ResolveSorafsModerationChallenge, SubmitSorafsModerationAppeal,
        SubmitSorafsModerationCommit, SubmitSorafsModerationReveal,
    },
    transaction::{Executable, SignedTransaction},
};
use super::SorafsModerationCommandRoute;
impl SorafsModerationCommandRoute {
    pub(super) const fn expected_instruction_label(self) -> &'static str {
        match self {
            Self::SubmitAppeal => "SubmitSorafsModerationAppeal",
            Self::RegisterEligibility => "RegisterSorafsModerationJurorEligibility",
            Self::FinalizeSortition => "FinalizeSorafsModerationSortition",
            Self::AcceptAssignment => "AcceptSorafsModerationJurorAssignment",
            Self::ActivateCase => "ActivateSorafsModerationCase",
            Self::SubmitCommit => "SubmitSorafsModerationCommit",
            Self::RaiseChallenge => "RaiseSorafsModerationChallenge",
            Self::ResolveChallenge => "ResolveSorafsModerationChallenge",
            Self::SubmitReveal => "SubmitSorafsModerationReveal",
            Self::FinalizeCase => "FinalizeSorafsModerationCase",
        }
    }
}
fn route_mismatch(route: SorafsModerationCommandRoute) -> eyre::Report {
    eyre!(
        "SoraFS moderation route requires exactly one `{}` native instruction",
        route.expected_instruction_label()
    )
}
/// Reject a transaction unless it contains the one native instruction selected by `route`.
pub(super) fn validate_transaction_route(
    route: SorafsModerationCommandRoute,
    transaction: &SignedTransaction,
) -> Result<()> {
    let Executable::Instructions(instructions) = transaction.instructions() else {
        return Err(route_mismatch(route));
    };
    let [instruction] = instructions.as_ref() else {
        return Err(route_mismatch(route));
    };
    let matches_route = super::repair::instruction_is!(instruction, route, {
        SorafsModerationCommandRoute::SubmitAppeal => SubmitSorafsModerationAppeal,
        SorafsModerationCommandRoute::RegisterEligibility => RegisterSorafsModerationJurorEligibility,
        SorafsModerationCommandRoute::FinalizeSortition => FinalizeSorafsModerationSortition,
        SorafsModerationCommandRoute::AcceptAssignment => AcceptSorafsModerationJurorAssignment,
        SorafsModerationCommandRoute::ActivateCase => ActivateSorafsModerationCase,
        SorafsModerationCommandRoute::SubmitCommit => SubmitSorafsModerationCommit,
        SorafsModerationCommandRoute::RaiseChallenge => RaiseSorafsModerationChallenge,
        SorafsModerationCommandRoute::ResolveChallenge => ResolveSorafsModerationChallenge,
        SorafsModerationCommandRoute::SubmitReveal => SubmitSorafsModerationReveal,
        SorafsModerationCommandRoute::FinalizeCase => FinalizeSorafsModerationCase,
        }
    );
    if !matches_route {
        return Err(route_mismatch(route));
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use std::num::NonZeroU64;
    use iroha_data_model::{
        Level,
        isi::{InstructionBox, Log, sorafs::SubmitSorafsModerationCommit},
        metadata::Metadata,
        transaction::{
            Executable, FeePaymentIntent, IvmBytecode, SignedTransaction, TransactionBuilder,
        },
    };
    use super::*;
    use crate::client::{
        SORAFS_MODERATION_TRANSACTION_TTL,
        evidence_http_tests::{base_url, client_with_base_url},
    };
    fn sign_executable(client: &super::super::Client, executable: Executable) -> SignedTransaction {
        let gas_limit = executable
            .requires_transaction_gas_limit()
            .then(|| NonZeroU64::new(1).expect("non-zero gas limit"));
        let mut builder = TransactionBuilder::new(
            client.network_id,
            client.account.clone(),
            FeePaymentIntent::authority(Vec::new(), gas_limit),
        )
        .with_executable(executable)
        .with_metadata(Metadata::default());
        builder.set_ttl(SORAFS_MODERATION_TRANSACTION_TTL);
        client
            .try_sign_transaction(builder)
            .expect("sign moderation route validation fixture")
    }
    fn assert_rejected_before_http(
        client: &super::super::Client,
        route: SorafsModerationCommandRoute,
        transaction: &SignedTransaction,
    ) {
        super::super::repair::route_test_support::assert_rejected_before_http(
            format!(
                "SoraFS moderation route requires exactly one `{}` native instruction",
                route.expected_instruction_label()
            ),
            || client.post_sorafs_moderation_transaction(route, transaction),
        );
    }
    #[test]
    fn moderation_route_validation_accepts_exact_instruction_and_rejects_mismatch_before_http() {
        let client = client_with_base_url(base_url());
        let transaction = client
            .try_build_sorafs_moderation_transaction(SubmitSorafsModerationCommit::new(vec![0xA5]))
            .expect("build exact commit transaction");
        validate_transaction_route(SorafsModerationCommandRoute::SubmitCommit, &transaction)
            .expect("matching moderation route");
        assert_rejected_before_http(
            &client,
            SorafsModerationCommandRoute::SubmitReveal,
            &transaction,
        );
    }
    #[test]
    fn moderation_route_validation_rejects_non_native_and_non_singleton_before_http() {
        let client = client_with_base_url(base_url());
        let wrong_instruction = client
            .try_build_sorafs_moderation_transaction(Log::new(
                Level::INFO,
                "not a moderation instruction".into(),
            ))
            .expect("build wrong-instruction transaction");
        assert_rejected_before_http(
            &client,
            SorafsModerationCommandRoute::SubmitCommit,
            &wrong_instruction,
        );
        let commit: InstructionBox = SubmitSorafsModerationCommit::new(vec![0xA5]).into();
        let multiple = sign_executable(
            &client,
            Executable::Instructions(vec![commit.clone(), commit].into()),
        );
        assert_rejected_before_http(
            &client,
            SorafsModerationCommandRoute::SubmitCommit,
            &multiple,
        );
        let ivm = sign_executable(
            &client,
            Executable::Ivm(IvmBytecode::from_compiled(vec![0x00])),
        );
        assert_rejected_before_http(&client, SorafsModerationCommandRoute::SubmitCommit, &ivm);
    }
}
