//! Native account-custody execution and state-integrity regressions.
//! Device attestations are simulated signatures; no hardware or production-finality claim is made.
use super::*;
use crate::{
    query::final_promotion_account_custody::read_final_promotion_account_custody_at_v1,
    state::WorldReadOnly,
};
use iroha_data_model::{
    permission::{Permission, Permissions},
    sorafs::final_promotion_account_custody::{
        FINAL_PROMOTION_ACCOUNT_CUSTODY_MAX_RECORD_BYTES_V1, FinalPromotionAccountCustodyCheckV1,
        FinalPromotionAccountCustodyRevocationV1,
    },
};
use iroha_executor_data_model::permission::sorafs::{
    CanCheckSorafsFinalPromotionAccountCustody, CanOperateSorafsFinalPromotion,
};
use iroha_model_base::state_path::StatePath;
use mv::storage::StorageReadOnly;
use sorafs_manifest::signer::custody_control::SignerCustodyPolicyV1;

mod fixture;
use fixture::*;
mod capacity;
mod check_tests;
mod history_tests;

fn retained(tx: &StateTransaction<'_, '_>) -> Vec<(StatePath, Vec<u8>)> {
    tx.world
        .smart_contract_state
        .iter()
        .map(|(key, bytes)| (key.clone(), bytes.clone()))
        .collect()
}
fn enrolled() -> Fixture {
    let mut f = fixture();
    configure(&mut f);
    enroll(&mut f);
    f
}
fn check_instruction(f: &Fixture) -> MutateSorafsFinalPromotionAccountCustody {
    let current = snapshot(f);
    MutateSorafsFinalPromotionAccountCustody {
        deployment_id: DEPLOYMENT.into(),
        expected_control_revision: current.control_record.revision,
        expected_control_digest: current.custody_anchor.state_digest,
        action: Action::Check(FinalPromotionAccountCustodyCheckV1 {
            challenge: [90; 32],
            network_id: f.policy.binding.network_id,
            minimum_height: current.custody_anchor.height,
            minimum_block_hash: current.custody_anchor.block_hash,
            expected_account: f.target.clone(),
            transaction_payload_digest: [91; 32],
        }),
    }
}
fn payload_mut(
    instruction: &mut MutateSorafsFinalPromotionAccountCustody,
) -> &mut FinalPromotionAccountCustodyCheckV1 {
    let Action::Check(check) = &mut instruction.action else {
        panic!("Check fixture")
    };
    check
}
fn assert_no_writes(
    tx: &mut StateTransaction<'_, '_>,
    instruction: &MutateSorafsFinalPromotionAccountCustody,
    authority: &AccountId,
    allowed: bool,
) {
    let before = retained(tx);
    assert_eq!(instruction.clone().execute(authority, tx).is_ok(), allowed);
    assert_eq!(
        retained(tx),
        before,
        "native Check must not mutate any state"
    );
}
fn applied(
    f: &Fixture,
    instruction: &MutateSorafsFinalPromotionAccountCustody,
    now: u64,
) -> Result<FinalPromotionAccountCustodySnapshotV1, HistoryError> {
    check::check_applied_snapshot_v1(
        &f.state.view(),
        instruction,
        &f.policy.binding,
        &f.observer,
        now,
    )
}
