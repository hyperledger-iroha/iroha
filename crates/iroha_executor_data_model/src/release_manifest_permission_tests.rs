//! Role-13 permissions have exact deployment scope and cannot become another signer role.
use crate::permission::{
    Permission as _,
    sorafs::{
        CanCheckSorafsFinalPromotion, CanCheckSorafsReleaseManifest,
        CanManageSorafsFinalPromotionCustody, CanManageSorafsReleaseManifestCustody,
        CanOperateSorafsFinalPromotion, CanOperateSorafsReleaseManifest,
    },
};
use iroha_data_model::permission::Permission;

#[test]
fn release_manifest_permissions_have_closed_json_and_canonical_identity() {
    macro_rules! check {
        ($type:ident) => {{
            let typed = $type {
                deployment_id: "release-primary".into(),
            };
            assert_eq!($type::name(), stringify!($type));
            assert_eq!(
                norito::json::to_json(&typed).expect("permission JSON"),
                r#"{"deployment_id":"release-primary"}"#
            );
            let token: Permission = typed.clone().into();
            let frame = norito::encode_canonical(&token).expect("permission frame");
            let decoded: Permission = norito::decode_canonical(&frame).expect("permission decode");
            assert_eq!(decoded, token);
            assert_eq!($type::try_from(&decoded).expect("exact typed token"), typed);
            for malformed in [
                "{}",
                r#"{"deployment_id":null}"#,
                r#"{"deployment_id":3}"#,
                r#"{"deploymentId":"release-primary"}"#,
                r#"{"deployment_id":"release-primary","extra":1}"#,
                r#"{"deployment_id":"release-primary","deployment_id":"other"}"#,
            ] {
                assert!(norito::json::from_str::<$type>(malformed).is_err());
            }
        }};
    }
    check!(CanManageSorafsReleaseManifestCustody);
    check!(CanOperateSorafsReleaseManifest);
    check!(CanCheckSorafsReleaseManifest);
}

#[test]
fn release_manifest_grants_reject_foreign_role_and_deployment() {
    let deployment_id = "release-primary".to_owned();
    let tokens: [Permission; 6] = [
        CanManageSorafsReleaseManifestCustody {
            deployment_id: deployment_id.clone(),
        }
        .into(),
        CanOperateSorafsReleaseManifest {
            deployment_id: deployment_id.clone(),
        }
        .into(),
        CanCheckSorafsReleaseManifest {
            deployment_id: deployment_id.clone(),
        }
        .into(),
        CanManageSorafsFinalPromotionCustody {
            deployment_id: deployment_id.clone(),
        }
        .into(),
        CanOperateSorafsFinalPromotion {
            deployment_id: deployment_id.clone(),
        }
        .into(),
        CanCheckSorafsFinalPromotion { deployment_id }.into(),
    ];
    for (index, token) in tokens.iter().enumerate() {
        assert_eq!(
            CanManageSorafsReleaseManifestCustody::try_from(token).is_ok(),
            index == 0
        );
        assert_eq!(
            CanOperateSorafsReleaseManifest::try_from(token).is_ok(),
            index == 1
        );
        assert_eq!(
            CanCheckSorafsReleaseManifest::try_from(token).is_ok(),
            index == 2
        );
        for other in tokens.iter().skip(index + 1) {
            assert_ne!(token, other);
        }
    }
    let foreign: Permission = CanOperateSorafsReleaseManifest {
        deployment_id: "release-secondary".into(),
    }
    .into();
    assert_ne!(tokens[1], foreign);
}
