//! Selection of the distinct G3 runtime enactment result owner (N14).
//!
//! A root selects the owner only when its selection installs the anonymous Parliament family
//! and its exact compiled services profile names
//! [`RUNTIME_ENACTMENT_RESULT_SERVICE_V1`] with exactly the reference closure of
//! [`RUNTIME_ENACTMENT_RESULT_SERVICE_CLOSURE_V1`]. A matching name with another closure is
//! refused, never treated as absence.
//!
//! This release composes the service into no services profile, so every root answers "not
//! selected" and G3 application keeps its original refusal before effects.
//! TODO(N14): compose the service only together with block publication of the admitted
//! results and the qualified ballot authority (P1).

use iroha_data_model::governance::anonymous_ballot::{
    RUNTIME_ENACTMENT_RESULT_SERVICE_CLOSURE_V1, RUNTIME_ENACTMENT_RESULT_SERVICE_V1,
};

use super::*;

/// The exact reference closure digest of the G3 result service.
fn reference_closure() -> [u8; 32] {
    iroha_crypto::Hash::new(RUNTIME_ENACTMENT_RESULT_SERVICE_CLOSURE_V1).into()
}

/// Whether `selection` selects the G3 result owner under `catalog`.
///
/// # Errors
/// The selection installs storage this release does not implement, its services profile is
/// not a compiled services descriptor, or the profile names the service with another closure.
pub(crate) fn selected_in(
    catalog: &ReleaseCatalog,
    selection: &RootSelectionV1,
) -> Result<bool, String> {
    if !crate::runtime::parliament_anonymous_installed(selection)? {
        return Ok(false);
    }
    catalog
        .service_enabled(
            selection.services_profile,
            RUNTIME_ENACTMENT_RESULT_SERVICE_V1,
            reference_closure(),
        )
        .map_err(|error| error.to_string())
}

/// [`selected_in`] under this release's catalog.
///
/// # Errors
/// The release catalog cannot be built, or as [`selected_in`].
pub(crate) fn selected(selection: &RootSelectionV1) -> Result<bool, String> {
    selected_in(
        release_catalog().map_err(|error| error.to_string())?,
        selection,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{World, WorldReadOnly as _};

    /// The selection of a World on which the anonymous Parliament family is active.
    fn family_selection() -> RootSelectionV1 {
        let world = World::new();
        crate::runtime::activate_additions_for_testing(
            &world,
            additions::PARLIAMENT_ANONYMOUS.module,
        )
        .expect("activate the anonymous family");
        world.view().runtime_selection().clone()
    }

    /// `catalog` with a copy of `profile` that also names the G3 result service with
    /// `closure`, and that copy's identity.
    fn composed(
        catalog: &ReleaseCatalog,
        profile: DefinitionId,
        closure: [u8; 32],
    ) -> (ReleaseCatalog, DefinitionId) {
        let mut catalog = catalog.clone();
        let Some(DefinitionDescriptorV1::ServicesProfile(mut services)) =
            catalog.definitions.get(&profile).cloned()
        else {
            panic!("the selection's services profile is compiled");
        };
        services.services.push(ServiceEntryV1 {
            name: RUNTIME_ENACTMENT_RESULT_SERVICE_V1.into(),
            reference_closure: closure,
        });
        services.services.sort_by(|a, b| a.name.cmp(&b.name));
        let id = catalog
            .publish(DefinitionDescriptorV1::ServicesProfile(services))
            .expect("publish the composed profile");
        (catalog, id)
    }

    #[test]
    fn no_compiled_services_profile_selects_the_g3_result_owner() {
        let catalog = release_catalog().expect("release catalog");
        let mut profiles = 0;
        for descriptor in catalog.definitions.values() {
            let DefinitionDescriptorV1::ServicesProfile(services) = descriptor else {
                continue;
            };
            profiles += 1;
            assert!(
                services
                    .services
                    .iter()
                    .all(|entry| entry.name.as_str() != RUNTIME_ENACTMENT_RESULT_SERVICE_V1),
                "composing the G3 result service requires its block publication and P1"
            );
        }
        assert!(profiles > 0);
        assert_eq!(selected(&catalog.baseline), Ok(false));
        assert_eq!(selected(&catalog.lifecycle), Ok(false));
        let family = family_selection();
        assert!(crate::runtime::parliament_anonymous_installed(&family).unwrap());
        assert_eq!(selected(&family), Ok(false));
    }

    #[test]
    fn an_exact_composed_profile_selects_the_owner_only_with_the_family() {
        let release = release_catalog().expect("release catalog");
        let mut family = family_selection();
        let (catalog, profile) = composed(release, family.services_profile, reference_closure());
        family.services_profile = profile;
        assert_eq!(selected_in(&catalog, &family), Ok(true));
        // The same profile on a root without the anonymous family selects nothing.
        let mut baseline = release.baseline.clone();
        baseline.services_profile = profile;
        assert!(!crate::runtime::parliament_anonymous_installed(&baseline).unwrap());
        assert_eq!(selected_in(&catalog, &baseline), Ok(false));
        // The real catalog never compiled that profile.
        assert!(selected(&family).is_err());
    }

    #[test]
    fn another_closure_or_an_uncompiled_profile_is_refused_never_absent() {
        let release = release_catalog().expect("release catalog");
        let mut family = family_selection();
        let original = family.services_profile;
        let (catalog, profile) = composed(release, original, [0x5a; 32]);
        family.services_profile = profile;
        assert!(selected_in(&catalog, &family).is_err());
        // A kernel identity is not a services descriptor.
        family.services_profile = family.kernel;
        assert!(selected_in(release, &family).is_err());
        family.services_profile = original;
        assert_eq!(selected_in(release, &family), Ok(false));
    }
}
