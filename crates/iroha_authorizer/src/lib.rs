//! A crate enabling user-defined logic to authorize or reject executables and queries based on the authority’s permissions and ownerships.
//!
//! This is a stripped-down version of the executor, focused solely on permission validation.
//! It does not define or execute instructions.

#![no_std]

extern crate alloc;
use alloc::rc::Rc;
use core::ops::BitOr;

use iroha_tree::{dm, event, fuzzy_node, permission, readset, some, state, Filtered};

/// User-defined logic responsible for permission validation:
///
/// - Validates write access using event predictions.
/// - Performs post-execution validation for read access to world entities using actual events.
/// - Performs post-execution validation for read access to transactions using events derived from transaction executables.
pub trait Authorizer {
    /// Query declaration to retrieve the data required for authorization.
    fn context_request(&self) -> readset::ReadSet;

    /// Authorizes or rejects read or write requests based on events and contexts, which typically include the authority's permissions.
    ///
    /// # Errors
    ///
    /// Fails if the event-based access attempt is determined to exceed the given context's permissions.
    fn authorize(
        &self,
        event: &event::Event,
        context: &state::StateView,
    ) -> Result<(), PermissionDenied>;
}

/// Indicates an authorization failure.
/// In post-execution validation for queries, this error should not reveal information about data existence.
pub enum PermissionDenied {
    /// Informs that authorization could have passed if this permission were present.
    MissingPermission(permission::Permission),
    /// Only indicates that the authorization has failed.
    Forbidden,
}

// TODO: Implement equivalent logic to the following in Wasm.

/// A sample authorizer with standard behavior.
#[derive(Debug)]
pub struct DefaultAuthorizer {
    authority: dm::AccountId,
}

impl Authorizer for DefaultAuthorizer {
    fn context_request(&self) -> readset::ReadSet {
        use readset::UnitR;

        let key = (
            some!(self.authority.signatory.clone()),
            some!(self.authority.domain.clone()),
        );
        let (acc, dom) = (|| key.0.clone(), || key.1.clone());

        // TODO: The `state::WorldState::load` should complete relevant primary entities, similar to the data integrity check in `state::WorldState::sanitize`.
        readset::ReadSet::from_iter([
            fuzzy_node!(AccountRole, acc(), dom(), None, UnitR),
            fuzzy_node!(AccountPermission, acc(), dom(), None, UnitR),
            fuzzy_node!(DomainAdmin, None, acc(), dom(), UnitR),
            fuzzy_node!(AssetAdmin, None, None, acc(), dom(), UnitR),
            fuzzy_node!(NftAdmin, None, None, acc(), dom(), UnitR),
            fuzzy_node!(NftOwner, None, None, acc(), dom(), UnitR),
            fuzzy_node!(TriggerAdmin, None, acc(), dom(), UnitR),
        ])
    }

    fn authorize(
        &self,
        event: &event::Event,
        _context: &state::StateView,
    ) -> Result<(), PermissionDenied> {
        // TODO: Implement data retrieval from `context`.
        let role_permission = permission::Permission::default();
        let account_permission = permission::Permission::default();
        let domain_admin = permission::Permission::default();
        let asset_admin = permission::Permission::default();
        let nft_admin = permission::Permission::default();
        let nft_owner = permission::Permission::default();
        let trigger_admin = permission::Permission::default();

        let permission = [
            account_permission,
            role_permission,
            domain_admin,
            asset_admin,
            nft_admin,
            nft_owner,
            trigger_admin,
        ]
        .into_iter()
        .reduce(BitOr::bitor)
        .unwrap();

        event
            .passes(&permission)
            .map_err(PermissionDenied::MissingPermission)
    }
}
