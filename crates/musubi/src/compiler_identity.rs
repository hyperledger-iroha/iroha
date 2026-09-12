//! Canonical Kotodama identities for authenticated Musubi package graphs.
//!
//! Display labels are diagnostic text and do not satisfy the compiler's identity grammar.
//! Hash length-delimited structural fields under one fixed domain, distinguishing local
//! source packages from registry releases. Import aliases and filesystem paths are not
//! part of a package's nominal type identity.

use iroha_data_model::musubi::{
    MusubiPackageScopeV1, MusubiPackageSelectorV1, MusubiReleaseIdV1, MusubiVersionV1,
};

const IDENTITY_CONTEXT: &str = "iroha.musubi.kotodama-package-identity.v1";

pub fn local_package(selector: &MusubiPackageSelectorV1, version: &MusubiVersionV1) -> String {
    identity(
        "local",
        &[
            selector.namespace.as_str().as_bytes(),
            selector.name.as_str().as_bytes(),
            version.to_string().as_bytes(),
        ],
    )
}

pub fn registry_release(release: &MusubiReleaseIdV1) -> String {
    let (scope, domain) = match &release.package.scope {
        MusubiPackageScopeV1::DataspaceRoot => ("root", ""),
        MusubiPackageScopeV1::Domain(domain) => ("domain", domain.as_ref()),
    };
    identity(
        "registry",
        &[
            &release.package.home_dataspace.as_u64().to_le_bytes(),
            scope.as_bytes(),
            domain.as_bytes(),
            release.package.name.as_str().as_bytes(),
            release.version.to_string().as_bytes(),
        ],
    )
}

fn identity(kind: &str, fields: &[&[u8]]) -> String {
    let mut hasher = blake3::Hasher::new_derive_key(IDENTITY_CONTEXT);
    for field in std::iter::once(kind.as_bytes()).chain(fields.iter().copied()) {
        hasher.update(&(field.len() as u64).to_le_bytes());
        hasher.update(field);
    }
    format!("musubi/{kind}/{}", hasher.finalize().to_hex())
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_data_model::{
        musubi::MusubiPackageIdV1,
        smart_contract::entrypoint::is_canonical_kotodama_package_identity,
    };
    use iroha_model_base::topology::DataSpaceId;
    use std::collections::BTreeSet;

    #[test]
    fn local_identity_is_canonical_and_binds_each_component() {
        let selector: MusubiPackageSelectorV1 = "apps.sora/demo".parse().expect("selector");
        let version = "1.2.3-rc.1".parse().expect("version");
        let original = local_package(&selector, &version);
        assert!(is_canonical_kotodama_package_identity(&original));
        assert_eq!(original, local_package(&selector.clone(), &version.clone()));
        for (selector, version) in [
            ("other.sora/demo", "1.2.3-rc.1"),
            ("apps.sora/other", "1.2.3-rc.1"),
            ("apps.sora/demo", "1.2.3-rc.2"),
        ] {
            let changed = local_package(
                &selector.parse().expect("selector"),
                &version.parse().expect("version"),
            );
            assert!(is_canonical_kotodama_package_identity(&changed));
            assert_ne!(original, changed);
        }
    }

    #[test]
    fn registry_identity_binds_structural_scope_dataspace_name_and_version() {
        let base = MusubiReleaseIdV1::new(
            MusubiPackageIdV1::new(
                DataSpaceId::new(7),
                MusubiPackageScopeV1::Domain("apps".parse().expect("domain")),
                "demo".parse().expect("name"),
            ),
            "1.2.3-rc.1".parse().expect("version"),
        );
        let mut alternatives = vec![base.clone(); 5];
        alternatives[0].package.home_dataspace = DataSpaceId::new(8);
        alternatives[1].package.scope = MusubiPackageScopeV1::DataspaceRoot;
        alternatives[2].package.scope =
            MusubiPackageScopeV1::Domain("other".parse().expect("domain"));
        alternatives[3].package.name = "other".parse().expect("name");
        alternatives[4].version = "1.2.3-rc.2".parse().expect("version");
        let mut identities = BTreeSet::new();
        for release in std::iter::once(&base).chain(&alternatives) {
            release.validate().expect("canonical release");
            let value = registry_release(release);
            assert!(is_canonical_kotodama_package_identity(&value));
            assert!(identities.insert(value));
        }
        assert!(!identities.contains(&local_package(
            &"apps.sora/demo".parse().expect("selector"),
            &base.version,
        )));
        assert_eq!(registry_release(&base), registry_release(&base.clone()));
    }

    #[test]
    fn identity_frames_field_boundaries_and_source_kind() {
        assert_ne!(
            identity("local", &[b"ab", b"c"]),
            identity("local", &[b"a", b"bc"])
        );
        assert_ne!(identity("local", &[b"a", b""]), identity("local", &[b"a"]));
        assert_ne!(identity("local", &[b"a"]), identity("registry", &[b"a"]));
    }
}
