//! Shared world fixture for `iroha_core` integration tests.

use iroha_core::state::World;
use iroha_data_model::Registrable;
use iroha_data_model::prelude::{Account, AssetDefinition, Domain};
use iroha_test_samples::{ALICE_ID, BOB_ID};

/// Build a minimal world with the standard test accounts.
pub(crate) fn world_with_test_accounts() -> World {
    let domain_id: iroha_data_model::domain::DomainId = "wonderland".parse().expect("domain");
    let domain = Domain::new(domain_id.clone()).build(&ALICE_ID);
    let alice = Account::new_in_domain(ALICE_ID.clone(), domain_id.clone()).build(&ALICE_ID);
    let bob = Account::new_in_domain(BOB_ID.clone(), domain_id).build(&BOB_ID);
    World::with(
        [domain],
        [alice, bob],
        std::iter::empty::<AssetDefinition>(),
    )
}
