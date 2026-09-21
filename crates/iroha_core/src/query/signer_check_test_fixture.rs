//! Exact native execution and fixed-roster finality for cross-crate Check tests.
//!
//! Available only for Core tests or the existing `iroha-core-tests` feature. The fixture
//! owns its empty native history and fixed SCCP network, constructs aligned results through
//! actual native execution, and retains its own three-of-four BLS / RS16 parent chain.
//! Its software signatures and test frontier commits do not qualify deployed consensus,
//! production custody, a production application-state root or an independent floor store.
//! It bypasses ordinary admission and fees. Only custody, observer/operator permission and
//! account removal, role registration and Log instructions under the initial executor with an
//! empty trigger registry can enter its typed outputs.
use crate::{
    kura::Kura,
    query::{signer_check::fixture, store::LiveQueryStore},
    state::{State, StateReadOnly, World},
};
use iroha_data_model::{
    block::consensus_v2::HeightContextId,
    transaction::{Executable, SignedTransaction},
};
use iroha_sccp::{
    SCCP_TAIRA_CHAIN_ID_V1, SccpFinalizedBlockTestFixtureV1, sccp_taira_finality_network_id_v1,
};
use mv::storage::StorageReadOnly;
use std::sync::Arc;

/// Owns one empty-start native test State and its exact fixed-roster finalized parents.
///
/// No constructor accepts a finality artifact, retained history, Check success or verified
/// authority. Callers obtain real Check capabilities only through the public proof consumers.
pub struct NativeCheckTestFixtureV1 {
    state: Arc<State>,
    finalized: Vec<SccpFinalizedBlockTestFixtureV1>,
}

impl NativeCheckTestFixtureV1 {
    /// Execute account registration and scoped grants for a final-promotion test deployment.
    ///
    /// This is fixture setup, not genesis or finalized history. Manager receives the two custody
    /// management permissions, operator receives Operate, and observer receives both Checks.
    /// All custody policies, enrollments and operations must still execute through `commit`.
    ///
    /// # Panics
    /// Panics on equal role accounts, invalid setup instructions or unavailable fixture storage.
    #[must_use]
    pub fn with_final_promotion_accounts(
        deployment: &str,
        manager: iroha_data_model::account::AccountId,
        operator: iroha_data_model::account::AccountId,
        observer: iroha_data_model::account::AccountId,
    ) -> Self {
        use crate::smartcontracts::Execute;
        use iroha_data_model::{
            account::Account,
            block::BlockHeader,
            isi::{Grant, Register},
            permission::Permission,
        };
        use iroha_executor_data_model::permission::sorafs::{
            CanCheckSorafsFinalPromotion, CanCheckSorafsFinalPromotionAccountCustody,
            CanManageSorafsFinalPromotionAccountCustody, CanManageSorafsFinalPromotionCustody,
            CanOperateSorafsFinalPromotion,
        };
        assert!(manager != operator && manager != observer && operator != observer);
        let fixture = Self::new(World::new());
        let mut block = fixture.state.block(BlockHeader::new(
            std::num::NonZeroU64::MIN,
            None,
            None,
            0,
            0,
        ));
        let mut transaction = block.transaction();
        for id in [&manager, &operator, &observer] {
            Register::account(Account::new(id.clone()))
                .execute(&manager, &mut transaction)
                .expect("register native Check fixture account");
        }
        let grants: [(Permission, &iroha_data_model::account::AccountId); 5] = [
            (
                CanManageSorafsFinalPromotionCustody {
                    deployment_id: deployment.into(),
                }
                .into(),
                &manager,
            ),
            (
                CanManageSorafsFinalPromotionAccountCustody {
                    deployment_id: deployment.into(),
                }
                .into(),
                &manager,
            ),
            (
                CanOperateSorafsFinalPromotion {
                    deployment_id: deployment.into(),
                }
                .into(),
                &operator,
            ),
            (
                CanCheckSorafsFinalPromotion {
                    deployment_id: deployment.into(),
                }
                .into(),
                &observer,
            ),
            (
                CanCheckSorafsFinalPromotionAccountCustody {
                    deployment_id: deployment.into(),
                }
                .into(),
                &observer,
            ),
        ];
        for (permission, account) in grants {
            Grant::account_permission(permission, account.clone())
                .execute(&manager, &mut transaction)
                .expect("grant scoped native Check fixture permission");
        }
        transaction.apply();
        block
            .commit_world_overlay_for_testing()
            .expect("commit native Check fixture setup");
        fixture
    }

    /// Start the fixed test network from account/permission setup with no native custody rows.
    ///
    /// # Panics
    /// Panics if any contract-state history is supplied or a local test store cannot initialize.
    #[must_use]
    pub fn new(world: World) -> Self {
        assert!(
            world.smart_contract_state.view().iter().next().is_none(),
            "native Check fixtures must execute their own custody history"
        );
        Self {
            state: Arc::new(State::new_with_chain_and_network_id_for_testing(
                world,
                Kura::blank_kura_for_testing(),
                LiveQueryStore::start_test(),
                SCCP_TAIRA_CHAIN_ID_V1
                    .parse()
                    .expect("fixed test chain identity"),
                sccp_taira_finality_network_id_v1(),
            )),
            finalized: Vec::new(),
        }
    }

    /// Borrow the exact State used by native execution and the public Check proof consumers.
    #[must_use]
    pub fn state(&self) -> &Arc<State> {
        &self.state
    }

    /// Execute exact signed native entries and retain their actual outcomes and real test finality.
    ///
    /// This always records aligned membership and durable finality. It has no bypass flags.
    /// Failed native instructions retain their rejection result, with their transaction discarded.
    /// The existing SCCP fixture supports only heights 1 through 9 before its epoch boundary.
    ///
    /// # Panics
    /// Panics before execution for a foreign network, unsupported instruction or executor,
    /// registered trigger, invalid signature, altered State frontier or an exhausted fixture epoch.
    /// Panics on an internal fixture error.
    pub fn commit(&mut self, now: u64, transactions: Vec<SignedTransaction>) -> Vec<bool> {
        assert!(self.finalized.len() < 9, "fixed fixture epoch is exhausted");
        {
            let view = self.state.view();
            assert_eq!(
                view.height(),
                self.finalized.len(),
                "fixture frontier changed"
            );
            assert_eq!(
                view.latest_block_hash(),
                self.finalized.last().map(|parent| parent.block().hash()),
                "fixture parent changed"
            );
            assert_eq!(view.network_id(), &sccp_taira_finality_network_id_v1());
            assert_eq!(view.chain_id().to_string(), SCCP_TAIRA_CHAIN_ID_V1);
        }
        for transaction in &transactions {
            assert_eq!(transaction.network_id(), Some(self.state.network_id_ref()));
            assert!(matches!(
                transaction.instructions(),
                Executable::Instructions(_)
            ));
            transaction
                .verify_signature()
                .expect("exact signed fixture entry");
        }
        fixture::commit(
            &self.state,
            &mut self.finalized,
            now,
            transactions,
            true,
            true,
        )
    }

    /// Return coordinates from this fixture's own last signed parent, never from a candidate.
    ///
    /// `None` denotes the empty start. Callers copy these test coordinates into the distinct
    /// purpose-owned floor DTO before beginning a Check; they are not production floor trust.
    #[must_use]
    pub fn finalized_floor(&self) -> Option<(u64, [u8; 32], HeightContextId)> {
        self.finalized.last().map(|parent| {
            let artifact = &parent.proof().finality_artifact;
            (
                artifact.height,
                *artifact.block_hash.as_ref(),
                artifact.context_id(),
            )
        })
    }
}

#[cfg(test)]
mod tests;
