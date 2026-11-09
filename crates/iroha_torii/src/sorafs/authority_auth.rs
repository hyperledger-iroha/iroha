//! Controller-only HTTP authentication for the selected nondelegable SoraFS authority service.

use crate::app_auth::VerifiedCanonicalRequest;
use iroha_data_model::runtime::definition::RootSelectionV1;

/// A selected-service authentication check cannot establish the caller's operating authority.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum AuthorityRequestRejection {
    /// This build cannot interpret the exact committed services selection.
    UnsupportedSelection,
    /// An account's separately delegated off-chain key cannot exercise the selected authority.
    DelegatedSigningKey,
}

/// Require controller/witness authentication under the exact selected authority service.
///
/// The caller supplies the selection from the same native view used for its role or provider
/// policy check. Success confers no permission and must not replace those checks. The
/// predecessor's authentication behavior remains unchanged while the service is inactive.
///
/// # Errors
/// Refuses an unsupported services selection or a delegated key under the selected service.
pub(super) fn require_controller_authority(
    selection: &RootSelectionV1,
    verified: &VerifiedCanonicalRequest,
) -> Result<(), AuthorityRequestRejection> {
    let selected = iroha_core::runtime::sorafs_authority_enabled(selection)
        .map_err(|_| AuthorityRequestRejection::UnsupportedSelection)?;
    if selected && verified.offchain_signing_key {
        Err(AuthorityRequestRejection::DelegatedSigningKey)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_core::state::{World, WorldReadOnly};
    use iroha_data_model::runtime::definition::DefinitionId;

    fn request(delegated: bool) -> VerifiedCanonicalRequest {
        let signer = iroha_test_samples::ALICE_KEYPAIR.public_key().clone();
        VerifiedCanonicalRequest {
            account: iroha_test_samples::ALICE_ID.clone(),
            signer: signer.clone(),
            verified_signers: vec![signer],
            offchain_signing_key: delegated,
        }
    }

    #[test]
    fn authority_auth_preserves_predecessor_and_rejects_only_selected_delegation() {
        let world = World::new();
        {
            let view = world.view();
            for delegated in [false, true] {
                require_controller_authority(view.runtime_selection(), &request(delegated))
                    .expect("inactive predecessor keeps its authentication behavior");
            }
        }
        iroha_core::runtime::activate_additions_for_testing(
            &world,
            "iroha.sorafs.authority_governance.v1",
        )
        .unwrap();
        let view = world.view();
        assert!(iroha_core::runtime::sorafs_authority_enabled(view.runtime_selection()).unwrap());
        require_controller_authority(view.runtime_selection(), &request(false))
            .expect("verified native controller authentication passes this additional gate");
        assert_eq!(
            require_controller_authority(view.runtime_selection(), &request(true)),
            Err(AuthorityRequestRejection::DelegatedSigningKey)
        );
    }

    #[test]
    fn authority_auth_unknown_selection_never_falls_back_to_predecessor_rules() {
        let world = World::new();
        let mut selection = world.view().runtime_selection().clone();
        selection.services_profile = DefinitionId([0xF3; 32]);
        for delegated in [false, true] {
            assert_eq!(
                require_controller_authority(&selection, &request(delegated)),
                Err(AuthorityRequestRejection::UnsupportedSelection)
            );
        }
    }
}
