// Exact custody and signing-operation permissions are enforced even during genesis.
use iroha_data_model::{
    isi::sorafs::{
        MutateSorafsFinalPromotionAccountCustody, MutateSorafsFinalPromotionAuthority,
        MutateSorafsStreamTokenCustody,
    },
    sorafs::final_promotion_account_custody::FinalPromotionAccountCustodyActionV1,
    sorafs::final_promotion_authority::FinalPromotionAuthorityActionV1,
};
use iroha_executor_data_model::permission::sorafs::{
    CanCheckSorafsFinalPromotion, CanCheckSorafsFinalPromotionAccountCustody,
    CanManageSorafsFinalPromotionAccountCustody, CanManageSorafsFinalPromotionCustody,
    CanOperateSorafsFinalPromotion,
};

pub(super) fn visit_custody_instruction<V: Execute + Visit + ?Sized>(
    executor: &mut V,
    instruction: &InstructionBox,
) -> bool {
    let any = instruction.as_any();
    if let Some(mutation) = any.downcast_ref::<MutateSorafsStreamTokenCustody>() {
        visit_mutate_stream_token_custody(executor, mutation);
        return true;
    }
    if let Some(mutation) = any.downcast_ref::<MutateSorafsFinalPromotionAuthority>() {
        visit_mutate_final_promotion_authority(executor, mutation);
        return true;
    }
    if let Some(mutation) = any.downcast_ref::<MutateSorafsFinalPromotionAccountCustody>() {
        visit_mutate_final_promotion_account_custody(executor, mutation);
        return true;
    }
    false
}

/// Mutate stream-token custody only with the exact provider-scoped permission.
pub fn visit_mutate_stream_token_custody<V: Execute + Visit + ?Sized>(
    executor: &mut V,
    isi: &iroha_data_model::isi::sorafs::MutateSorafsStreamTokenCustody,
) {
    let permission =
        iroha_executor_data_model::permission::sorafs::CanManageSorafsStreamTokenCustody {
            provider_id: isi.provider_id,
        };
    if permission.is_owned_by(&executor.context().authority, executor.host()) {
        execute!(executor, isi);
    }
    deny!(
        executor,
        "Exact provider-scoped stream-token custody permission is required"
    );
}

/// Authorize final-promotion actions only with the exact deployment and action capability.
pub fn visit_mutate_final_promotion_authority<V: Execute + Visit + ?Sized>(
    executor: &mut V,
    isi: &MutateSorafsFinalPromotionAuthority,
) {
    let authority = &executor.context().authority;
    if let FinalPromotionAuthorityActionV1::Check(check) = &isi.action
        && authority == &check.expected_operator
    {
        deny!(
            executor,
            "Receipt observer must differ from the expected operator"
        );
    }
    let authorized = match &isi.action {
        FinalPromotionAuthorityActionV1::Configure(_)
        | FinalPromotionAuthorityActionV1::Enroll(_)
        | FinalPromotionAuthorityActionV1::Revoke(_) => CanManageSorafsFinalPromotionCustody {
            deployment_id: isi.deployment_id.clone(),
        }
        .is_owned_by(authority, executor.host()),
        FinalPromotionAuthorityActionV1::Reserve(_)
        | FinalPromotionAuthorityActionV1::Complete(_)
        | FinalPromotionAuthorityActionV1::Expire(_) => CanOperateSorafsFinalPromotion {
            deployment_id: isi.deployment_id.clone(),
        }
        .is_owned_by(authority, executor.host()),
        FinalPromotionAuthorityActionV1::Check(_) => CanCheckSorafsFinalPromotion {
            deployment_id: isi.deployment_id.clone(),
        }
        .is_owned_by(authority, executor.host()),
    };
    if authorized {
        execute!(executor, isi);
    }
    deny!(
        executor,
        "Exact deployment-scoped final-promotion action permission is required"
    );
}

/// Authorize account-custody management or observation with its separate exact capability.
pub fn visit_mutate_final_promotion_account_custody<V: Execute + Visit + ?Sized>(
    executor: &mut V,
    isi: &MutateSorafsFinalPromotionAccountCustody,
) {
    let authority = &executor.context().authority;
    if let FinalPromotionAccountCustodyActionV1::Check(check) = &isi.action
        && authority == &check.expected_account
    {
        deny!(
            executor,
            "Account-custody observer must differ from the target account"
        );
    }
    let authorized = match &isi.action {
        FinalPromotionAccountCustodyActionV1::Configure(_)
        | FinalPromotionAccountCustodyActionV1::Enroll(_)
        | FinalPromotionAccountCustodyActionV1::Revoke(_) => {
            CanManageSorafsFinalPromotionAccountCustody {
                deployment_id: isi.deployment_id.clone(),
            }
            .is_owned_by(authority, executor.host())
        }
        FinalPromotionAccountCustodyActionV1::Check(_) => {
            CanCheckSorafsFinalPromotionAccountCustody {
                deployment_id: isi.deployment_id.clone(),
            }
            .is_owned_by(authority, executor.host())
        }
    };
    if authorized {
        execute!(executor, isi);
    }
    deny!(
        executor,
        "Exact deployment-scoped account-custody action permission is required"
    );
}
