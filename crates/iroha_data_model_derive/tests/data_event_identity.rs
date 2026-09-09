//! Canonical identities for generated ledger data event enums.

#[test]
fn production_data_events_preserve_captured_identities() {
    fn assert_frame_contract<
        T: norito::NoritoSerialize + for<'de> norito::NoritoDeserialize<'de>,
    >() {
    }

    macro_rules! check {
        ($ty:ty, $nominal:literal, $hash:literal) => {
            assert_eq!(<$ty as norito::NoritoSchema>::nominal_name(), $nominal);
            assert_eq!(<$ty as norito::NoritoSchema>::frame_name(), $nominal);
            let actual = norito::schema::identity::frame_hash::<$ty>();
            let rendered = format!("{:032x}", u128::from_be_bytes(actual));
            assert_eq!(rendered, $hash);
            assert_frame_contract::<$ty>();
        };
    }
    check!(
        iroha_data_model::events::data::prelude::AccountEvent,
        "iroha_data_model::events::data::events::account::AccountEvent",
        "db18c88fefff3a631a1a41f7a1cd6339"
    );
    check!(
        iroha_data_model::events::data::prelude::AccountRecoveryEvent,
        "iroha_data_model::events::data::events::account::AccountRecoveryEvent",
        "cf653dd79a09dd62ecdac3e36cb692c0"
    );
    check!(
        iroha_data_model::events::data::prelude::AssetDefinitionEvent,
        "iroha_data_model::events::data::events::asset::AssetDefinitionEvent",
        "11034f4bd49cc4141a7810c986f1b170"
    );
    check!(
        iroha_data_model::events::data::prelude::AssetEvent,
        "iroha_data_model::events::data::events::asset::AssetEvent",
        "d165a1ca62eb7a3082edcf667fb8eac1"
    );
    check!(
        iroha_data_model::events::data::prelude::BridgeEvent,
        "iroha_data_model::events::data::events::bridge::BridgeEvent",
        "49bbc190c9a141b41d7c878d1fffa1cc"
    );
    check!(
        iroha_data_model::events::data::prelude::DomainEvent,
        "iroha_data_model::events::data::events::domain::DomainEvent",
        "e2808b61348fbd17fa6bad14ce1839cc"
    );
    check!(
        iroha_data_model::events::data::prelude::NftEvent,
        "iroha_data_model::events::data::events::nft::NftEvent",
        "bc70ac5ba9a7fa3b30d4398c845872fd"
    );
    check!(
        iroha_data_model::events::data::prelude::PeerEvent,
        "iroha_data_model::events::data::events::peer::PeerEvent",
        "0efc0d538275b31036aba8ac5a0b11d7"
    );
    check!(
        iroha_data_model::events::data::prelude::RepoAccountEvent,
        "iroha_data_model::events::data::events::repo_account::RepoAccountEvent",
        "eae66f4d2c571836599270a993c417a3"
    );
    check!(
        iroha_data_model::events::data::prelude::RoleEvent,
        "iroha_data_model::events::data::events::role::RoleEvent",
        "a8fe5c8d4e8c93fff60a8d90f344b5d2"
    );
    check!(
        iroha_data_model::events::data::prelude::RwaEvent,
        "iroha_data_model::events::data::events::rwa::RwaEvent",
        "9f88f122848493e8aba6aa4b3426683e"
    );
    check!(
        iroha_data_model::events::data::prelude::TriggerEvent,
        "iroha_data_model::events::data::events::trigger::TriggerEvent",
        "3d5f39da65ee92de0c519bbc9fc11bb3"
    );
}
