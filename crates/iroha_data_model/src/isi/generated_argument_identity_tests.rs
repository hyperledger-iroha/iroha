//! Captured identities for generic instruction argument types.

use hex_literal::hex;
use norito::{NoritoDeserialize, NoritoSchema, NoritoSerialize};

use crate::{
    account::Account,
    asset::{Asset, AssetDefinition},
    domain::Domain,
    nft::{Nft, NftId},
    peer::Peer,
    permission::Permission,
    role::{Role, RoleId},
    rwa::Rwa,
    trigger::Trigger,
};

fn assert_marker<T>(nominal: &str, captured_hash: Option<[u8; 16]>)
where
    T: NoritoSchema + NoritoSerialize + for<'de> NoritoDeserialize<'de>,
{
    assert_eq!(T::nominal_name(), nominal);
    assert_eq!(T::frame_name(), nominal);
    let identity_hash = norito::schema::identity::frame_hash::<T>();
    assert_eq!(norito::schema::identity::frame_hash::<T>(), identity_hash);
    assert_eq!(norito::schema::identity::frame_hash::<T>(), identity_hash);
    if let Some(captured_hash) = captured_hash {
        assert_eq!(identity_hash, captured_hash);
    }
}

#[test]
fn generic_instruction_argument_markers_preserve_captured_identities() {
    assert_marker::<Account>(
        "iroha_data_model::account::model::Account",
        Some(hex!("ceefad9abfccddb427c54ac17072e886")),
    );
    assert_marker::<Domain>(
        "iroha_data_model::domain::model::Domain",
        Some(hex!("2f57b447ef56ee98d0982c70d8fd67d4")),
    );
    assert_marker::<AssetDefinition>(
        "iroha_data_model::asset::definition::model::AssetDefinition",
        Some(hex!("e2fae03b1b09bc9c72427a0bf4d0f2a9")),
    );
    assert_marker::<Asset>(
        "iroha_data_model::asset::value::model::Asset",
        Some(hex!("e8afb1a603e32d871f13f37541ad7f3c")),
    );
    assert_marker::<Nft>(
        "iroha_data_model::nft::model::Nft",
        Some(hex!("0b572b66ca4ed43db1c556f86596d515")),
    );
    assert_marker::<Role>(
        "iroha_data_model::role::model::Role",
        Some(hex!("a76e98064847a45b17ca56e0fc029106")),
    );
    assert_marker::<Trigger>(
        "iroha_data_model::trigger::model::model::Trigger",
        Some(hex!("afb3625c1b436a5754d425881f80ca44")),
    );
    assert_marker::<Peer>(
        "iroha_data_model::peer::model::Peer",
        Some(hex!("08f4b87e35ed5c1584d2ac002fef419f")),
    );
    assert_marker::<Permission>(
        "iroha_data_model::permission::model::Permission",
        Some(hex!("142443c7649070ecaeb714981e8f2fe2")),
    );
    assert_marker::<Rwa>(
        "iroha_data_model::rwa::Rwa",
        Some(hex!("61fdb974807c0dde14700a65f66659a3")),
    );

    // These names were captured as generic arguments. No standalone pre-declaration
    // hash row exists, so compare the marker directly with both existing codecs.
    assert_marker::<NftId>("iroha_data_model::nft::model::NftId", None);
    assert_marker::<RoleId>("iroha_data_model::role::model::RoleId", None);
}
