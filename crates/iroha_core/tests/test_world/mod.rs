//! Shared world fixture for ZK integration tests.

use iroha_core::state::World;
use iroha_data_model::{
    Registrable,
    prelude::{Account, AssetDefinition, Domain},
};
use iroha_test_samples::ALICE_ID;

/// Construct a minimal world containing the standard sample account.
pub(crate) fn world_with_test_accounts() -> World {
    let authority = ALICE_ID.clone();
    let wonderland = Domain::new(authority.domain().clone()).build(&authority);
    let alice = Account::new(authority.clone()).build(&authority);
    World::with([wonderland], [alice], std::iter::empty::<AssetDefinition>())
}
