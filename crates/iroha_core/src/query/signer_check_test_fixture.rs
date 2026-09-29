//! Exact native execution on a certified test chain for cross-crate Check tests.
//!
//! Available only for Core tests or the existing `iroha-core-tests` feature. The fixture owns a
//! [`CertifiedTestChain`]: a signed genesis with a fixed four-validator committee and blocks built,
//! executed and applied through the node's block path, each certified by a real BLS `CommitQC`, so
//! the Check proof consumers accept its history as they accept a running node's. Its software
//! signatures do not qualify deployed consensus, production custody, a production
//! application-state root or an independent floor store. Account and permission setup is the
//! initial World, never finalized history.
use crate::{
    query::signer_check::fixture,
    state::{State, World},
    sumeragi::test_chain::CertifiedTestChain,
};
use iroha_data_model::{block::consensus::HeightContextId, transaction::SignedTransaction};
use mv::storage::StorageReadOnly;
use std::sync::Arc;

/// Owns one native test chain whose custody history starts empty.
///
/// No constructor accepts a finality artifact, retained history, Check success or verified
/// authority. Callers obtain real Check capabilities only through the public proof consumers.
pub struct NativeCheckTestFixtureV1 {
    chain: CertifiedTestChain,
    state: Arc<State>,
}

impl NativeCheckTestFixtureV1 {
    /// Register the three role accounts and grant their scoped final-promotion permissions in
    /// the initial World of a new chain.
    ///
    /// This is fixture setup, not genesis or finalized history. Manager receives the two custody
    /// management permissions, operator receives Operate, and observer receives both Checks.
    /// All custody policies, enrollments and operations must still execute through `commit`.
    ///
    /// # Panics
    /// Panics on equal role accounts or when the chain cannot start.
    #[must_use]
    pub fn with_final_promotion_accounts(
        deployment: &str,
        manager: iroha_data_model::account::AccountId,
        operator: iroha_data_model::account::AccountId,
        observer: iroha_data_model::account::AccountId,
    ) -> Self {
        use iroha_data_model::{
            IntoKeyValue, Registrable,
            account::Account,
            permission::{Permission, Permissions},
        };
        use iroha_executor_data_model::permission::sorafs::{
            CanCheckSorafsFinalPromotion, CanCheckSorafsFinalPromotionAccountCustody,
            CanManageSorafsFinalPromotionAccountCustody, CanManageSorafsFinalPromotionCustody,
            CanOperateSorafsFinalPromotion,
        };
        assert!(manager != operator && manager != observer && operator != observer);
        let mut world = World::new();
        for id in [&manager, &operator, &observer] {
            let (id, account) = Account::new(id.clone()).build(&manager).into_key_value();
            world.accounts.insert(id, account);
        }
        let grants: [(&iroha_data_model::account::AccountId, Vec<Permission>); 3] = [
            (
                &manager,
                vec![
                    CanManageSorafsFinalPromotionCustody {
                        deployment_id: deployment.into(),
                    }
                    .into(),
                    CanManageSorafsFinalPromotionAccountCustody {
                        deployment_id: deployment.into(),
                    }
                    .into(),
                ],
            ),
            (
                &operator,
                vec![
                    CanOperateSorafsFinalPromotion {
                        deployment_id: deployment.into(),
                    }
                    .into(),
                ],
            ),
            (
                &observer,
                vec![
                    CanCheckSorafsFinalPromotion {
                        deployment_id: deployment.into(),
                    }
                    .into(),
                    CanCheckSorafsFinalPromotionAccountCustody {
                        deployment_id: deployment.into(),
                    }
                    .into(),
                ],
            ),
        ];
        for (account, grants) in grants {
            let mut permissions = Permissions::new();
            permissions.extend(grants);
            world
                .account_permissions
                .insert(account.clone(), permissions);
        }
        Self::new(world)
    }

    /// Start the test chain from account/permission setup with no native custody rows.
    ///
    /// # Panics
    /// Panics if any contract-state history is supplied or the chain cannot start.
    #[must_use]
    pub fn new(world: World) -> Self {
        assert!(
            world.smart_contract_state.view().iter().next().is_none(),
            "native Check fixtures must execute their own custody history"
        );
        let chain = fixture::chain(world);
        let state = Arc::clone(chain.state());
        Self { chain, state }
    }

    /// Borrow the exact State used by native execution and the public Check proof consumers.
    #[must_use]
    pub fn state(&self) -> &Arc<State> {
        &self.state
    }

    /// Execute signed transactions in one certified block at `now` and return whether each
    /// executed successfully (failed transactions keep their rejection result in the block).
    ///
    /// # Panics
    /// Panics before any publication for a transaction the chain does not accept (for example,
    /// another network's); panics if the block does not execute.
    pub fn commit(&mut self, now: u64, transactions: Vec<SignedTransaction>) -> Vec<bool> {
        fixture::commit(&mut self.chain, now, transactions)
    }

    /// Return coordinates of the chain's last certified block, never from a candidate: its
    /// height, block hash and certified block id.
    ///
    /// Callers copy these test coordinates into the distinct purpose-owned floor DTO before
    /// beginning a Check; they are not production floor trust. The signed genesis is the first
    /// floor.
    #[must_use]
    pub fn finalized_floor(&self) -> (u64, [u8; 32], HeightContextId) {
        let tip = self.chain.committed(self.chain.height());
        (tip.height(), *tip.block_hash().as_ref(), tip.id())
    }

    /// The certified chain.
    #[must_use]
    pub fn chain(&self) -> &CertifiedTestChain {
        &self.chain
    }
}

#[cfg(test)]
mod tests;
