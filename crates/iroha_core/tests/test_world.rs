//! Test helpers shared by ZK-focused integration tests.

use iroha_core::state::World;
use iroha_data_model::prelude::{Account, Domain};
use iroha_test_samples::{ALICE_ID, BOB_ID, SAMPLE_GENESIS_ACCOUNT_ID};

/// Build a minimal [`World`] containing the standard test accounts.
///
/// The resulting world includes:
/// - the `genesis` domain/account (for code paths that treat genesis specially)
/// - the `wonderland` domain with `alice@wonderland` and `bob@wonderland`
pub(crate) fn world_with_test_accounts() -> World {
    let genesis_account_id = (*SAMPLE_GENESIS_ACCOUNT_ID).clone();
    let genesis_domain =
        Domain::new(iroha_genesis::GENESIS_DOMAIN_ID.clone()).build(&genesis_account_id);
    let genesis_account = Account::new(genesis_account_id.clone()).build(&genesis_account_id);

    let wonderland_domain: Domain = Domain::new(ALICE_ID.domain.clone()).build(&ALICE_ID);
    let alice_account: Account = Account::new(ALICE_ID.clone()).build(&ALICE_ID);
    let bob_account: Account = Account::new(BOB_ID.clone()).build(&ALICE_ID);

    World::with(
        [genesis_domain, wonderland_domain],
        [genesis_account, alice_account, bob_account],
        [],
    )
}
