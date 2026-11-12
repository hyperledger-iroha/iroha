//! Immutable original construction inputs shared by all execution scopes.
//!
//! These values previously existed as mutable block and transaction snapshots.
//! Keeping the existing block snapshots here removes transaction copies and makes
//! a temporary policy substitution impossible through the execution-scope API.
//! This is original input custody, not a policy digest or publication authority.
//! The content limits and router inputs of a root that recognizes them are its
//! committed policy (`runtime::committed_policy`), frozen from the predecessor.
//! TODO: migrate the remaining node-configuration families (oracle, governance)
//! into the existing committed policy owners. Immutable capture alone does not
//! make local policies consensus parameters or complete the keyed State
//! publication and recovery contract.

use super::*;

/// Original static execution policy captured once with a block's predecessor.
///
/// Blocks and transactions expose shared field access through Deref, and no
/// mutable access or replacement constructor. Configure State before acquiring
/// an execution scope. Local caches are not another source of policy authority.
///
/// A policy cannot be changed before publication, even temporarily:
/// ```compile_fail
/// fn alter(block: &mut iroha_core::state::StateBlock<'_>) {
///     block.gov.citizenship_bond_amount = 1_u64.into();
/// }
/// ```
/// Nor can a transaction replace its original policy with foreign settings:
/// ```compile_fail
/// fn replace(tx: &mut iroha_core::state::StateTransaction<'_, '_>,
///            foreign: iroha_config::parameters::actual::Governance) {
///     tx.gov = foreign;
/// }
/// ```
/// The bundle itself cannot be replaced through the block fields either:
/// ```compile_fail
/// fn replace(block: &mut iroha_core::state::StateBlock<'_>,
///            foreign: iroha_core::state::StaticExecutionPolicy) {
///     **std::ops::DerefMut::deref_mut(block) = foreign;
/// }
/// ```
/// A transaction cannot detach its original input lifetime:
/// ```compile_fail
/// fn detach(tx: &iroha_core::state::StateTransaction<'_, '_>)
///     -> &'static iroha_core::state::StaticExecutionPolicy {
///     std::ops::Deref::deref(tx)
/// }
/// ```
/// An equal-value cryptography allocation is still not a replaceable input:
/// ```compile_fail
/// fn replace(tx: &mut iroha_core::state::StateTransaction<'_, '_>) {
///     tx.crypto = std::sync::Arc::new((*tx.crypto).clone());
/// }
/// ```
#[expect(
    clippy::manual_non_exhaustive,
    reason = "The private field confines construction to this module, including within this crate"
)]
pub struct StaticExecutionPolicy {
    /// Oracle execution settings from the original construction.
    pub oracle: iroha_config::parameters::actual::Oracle,
    /// Exact cryptography snapshot retained at original construction.
    pub crypto: Arc<iroha_config::parameters::actual::Crypto>,
    /// Governance execution settings from the original construction.
    pub gov: iroha_config::parameters::actual::Governance,
    /// Content settings: this node's gateway resources with the bundle limits
    /// of the predecessor, committed when its root recognizes them.
    pub content: iroha_config::parameters::actual::Content,
    /// Settlement router inputs of the predecessor, committed when its root
    /// recognizes them.
    pub settlement: iroha_config::parameters::actual::Settlement,
    // Keep construction in this module, including from other modules in this crate.
    _original: (),
}

impl StaticExecutionPolicy {
    /// Move the existing construction snapshots into their immutable owner.
    ///
    /// `world` is the predecessor the block freezes: the content limits and
    /// router inputs of a root that recognizes them are its committed values.
    pub(super) fn capture(state: &State, world: &impl crate::state::WorldReadOnly) -> Self {
        let selection = world.runtime_selection();
        let parameters = world.parameters();
        Self {
            oracle: state.oracle.clone(),
            crypto: state.crypto(),
            gov: state.gov.clone(),
            content: crate::runtime::committed_policy::effective_content(
                &state.content,
                selection,
                parameters,
            ),
            settlement: crate::runtime::committed_policy::effective_settlement(
                &state.settlement,
                selection,
                parameters,
            ),
            _original: (),
        }
    }
}

#[cfg(test)]
mod tests;
