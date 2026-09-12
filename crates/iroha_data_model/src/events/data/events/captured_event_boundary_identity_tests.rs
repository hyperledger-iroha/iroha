//! Source-bound compiler identities for this event capability's existing owners.

use crate::events::captured_event_boundary_identity_tests::check;

// These noncapturing cases keep the literal capture order in static storage.
const CASES: &[fn()] = &[
    || {
        check::<super::asset::AssetChanged>(
            "iroha_data_model::events::data::events::asset::model::AssetChanged",
            "70a28150268696feefeb1d004ac08c42",
            "70a28150268696feefeb1d004ac08c42",
        )
    },
    || {
        check::<super::asset::AssetTransferred>(
            "iroha_data_model::events::data::events::asset::model::AssetTransferred",
            "dbee7a017f413e8de82d2d58e15d3580",
            "dbee7a017f413e8de82d2d58e15d3580",
        )
    },
    || {
        check::<super::asset::AssetBatchTransferRejectionCode>(
            "iroha_data_model::events::data::events::asset::model::AssetBatchTransferRejectionCode",
            "5cf8d70f2c26ce6a6c86d24cf4b68838",
            "5cf8d70f2c26ce6a6c86d24cf4b68838",
        )
    },
    || {
        check::<super::asset::AssetBatchTransferLegStatus>(
            "iroha_data_model::events::data::events::asset::model::AssetBatchTransferLegStatus",
            "a3c331ea304d774965b5640cb2dc9e57",
            "a3c331ea304d774965b5640cb2dc9e57",
        )
    },
    || {
        check::<super::asset::AssetBatchTransferRejection>(
            "iroha_data_model::events::data::events::asset::model::AssetBatchTransferRejection",
            "42e18969ff8ee2bb80c7c9f9b5777edc",
            "42e18969ff8ee2bb80c7c9f9b5777edc",
        )
    },
    || {
        check::<super::asset::AssetBatchTransferOutcome>(
            "iroha_data_model::events::data::events::asset::model::AssetBatchTransferOutcome",
            "38a17371024d8a11ea67347f4cac0d65",
            "38a17371024d8a11ea67347f4cac0d65",
        )
    },
    || {
        check::<super::asset::AssetDefinitionTotalQuantityChanged>(
            "iroha_data_model::events::data::events::asset::model::AssetDefinitionTotalQuantityChanged",
            "364a358449f7ddd582fc03fc053f9c35",
            "364a358449f7ddd582fc03fc053f9c35",
        )
    },
    || {
        check::<super::asset::AssetDefinitionOwnerChanged>(
            "iroha_data_model::events::data::events::asset::model::AssetDefinitionOwnerChanged",
            "2b14639af75a0d28094234e027a9500c",
            "2b14639af75a0d28094234e027a9500c",
        )
    },
    || {
        check::<super::asset::AssetDefinitionMintabilityChanged>(
            "iroha_data_model::events::data::events::asset::model::AssetDefinitionMintabilityChanged",
            "11f85e674871098a8795fe8d026265c8",
            "11f85e674871098a8795fe8d026265c8",
        )
    },
    || {
        check::<super::bridge::SccpReplayDeltaEventV1>(
            "iroha_data_model::events::data::events::bridge::SccpReplayDeltaEventV1",
            "84ecf82a9e2d067ebdb7ea2a866ce159",
            "84ecf82a9e2d067ebdb7ea2a866ce159",
        )
    },
    || {
        check::<super::nft::NftOwnerChanged>(
            "iroha_data_model::events::data::events::nft::model::NftOwnerChanged",
            "3a5aed346ad0e16abc6588f4fbfb6060",
            "3a5aed346ad0e16abc6588f4fbfb6060",
        )
    },
    || {
        check::<super::rwa::RwaOwnerChanged>(
            "iroha_data_model::events::data::events::rwa::model::RwaOwnerChanged",
            "26fb6e8c19beec1ec7e93aedb92bbb29",
            "26fb6e8c19beec1ec7e93aedb92bbb29",
        )
    },
    || {
        check::<super::rwa::RwaSplit>(
            "iroha_data_model::events::data::events::rwa::model::RwaSplit",
            "3951c9fe20efb3f795a757025f7c8838",
            "3951c9fe20efb3f795a757025f7c8838",
        )
    },
    || {
        check::<super::rwa::RwaMerged>(
            "iroha_data_model::events::data::events::rwa::model::RwaMerged",
            "824a05b3610beb736b2e0728d3615787",
            "824a05b3610beb736b2e0728d3615787",
        )
    },
    || {
        check::<super::rwa::RwaQuantityChanged>(
            "iroha_data_model::events::data::events::rwa::model::RwaQuantityChanged",
            "149450e72f921abec75bcd0926607db9",
            "149450e72f921abec75bcd0926607db9",
        )
    },
    || {
        check::<super::rwa::RwaHoldChanged>(
            "iroha_data_model::events::data::events::rwa::model::RwaHoldChanged",
            "1dc1e3717365bd5c01a7407ab4814f36",
            "1dc1e3717365bd5c01a7407ab4814f36",
        )
    },
    || {
        check::<super::rwa::RwaControlsChanged>(
            "iroha_data_model::events::data::events::rwa::model::RwaControlsChanged",
            "c7a3fb94a87e5b703f94616560ad69e3",
            "c7a3fb94a87e5b703f94616560ad69e3",
        )
    },
    || {
        check::<super::role::RolePermissionChanged>(
            "iroha_data_model::events::data::events::role::model::RolePermissionChanged",
            "ac10e1c917b02fc98ac42162d2bf7bc6",
            "ac10e1c917b02fc98ac42162d2bf7bc6",
        )
    },
    || {
        check::<super::account::AccountCreated>(
            "iroha_data_model::events::data::events::account::model::AccountCreated",
            "2abf835c3620d6dc8f4892b284f94220",
            "2abf835c3620d6dc8f4892b284f94220",
        )
    },
    || {
        check::<super::account::AccountPermissionChanged>(
            "iroha_data_model::events::data::events::account::model::AccountPermissionChanged",
            "1f77637dcc822d5b1c7fe7d648c71c1f",
            "1f77637dcc822d5b1c7fe7d648c71c1f",
        )
    },
    || {
        check::<super::account::AccountControllerReplaced>(
            "iroha_data_model::events::data::events::account::model::AccountControllerReplaced",
            "48d5f69737b1f7e094b92e512fb15288",
            "48d5f69737b1f7e094b92e512fb15288",
        )
    },
    || {
        check::<super::account::AccountRoleChanged>(
            "iroha_data_model::events::data::events::account::model::AccountRoleChanged",
            "341fd951c5e8b47900509c4044040a2e",
            "341fd951c5e8b47900509c4044040a2e",
        )
    },
    || {
        check::<super::account::AccountRecoveryPolicySet>(
            "iroha_data_model::events::data::events::account::model::AccountRecoveryPolicySet",
            "ca8121846ef387e2c5e36a86937d2d8d",
            "ca8121846ef387e2c5e36a86937d2d8d",
        )
    },
    || {
        check::<super::account::AccountRecoveryPolicyCleared>(
            "iroha_data_model::events::data::events::account::model::AccountRecoveryPolicyCleared",
            "3b533c9bd03a885f043315e5966e56d3",
            "3b533c9bd03a885f043315e5966e56d3",
        )
    },
    || {
        check::<super::account::AccountRecoveryProposed>(
            "iroha_data_model::events::data::events::account::model::AccountRecoveryProposed",
            "4f0adc0fc23216c08c5c6def0cea761b",
            "4f0adc0fc23216c08c5c6def0cea761b",
        )
    },
    || {
        check::<super::account::AccountRecoveryApproved>(
            "iroha_data_model::events::data::events::account::model::AccountRecoveryApproved",
            "40162ca676e64a1f278040eaa2fb1662",
            "40162ca676e64a1f278040eaa2fb1662",
        )
    },
    || {
        check::<super::account::AccountRecoveryCancelled>(
            "iroha_data_model::events::data::events::account::model::AccountRecoveryCancelled",
            "a419db57aacf18d396492b9e24d3302c",
            "a419db57aacf18d396492b9e24d3302c",
        )
    },
    || {
        check::<super::account::AccountRecoveryFinalized>(
            "iroha_data_model::events::data::events::account::model::AccountRecoveryFinalized",
            "55ceea28ea2a8310839ce0273923b281",
            "55ceea28ea2a8310839ce0273923b281",
        )
    },
    || {
        check::<super::repo_account::RepoAccountRole>(
            "iroha_data_model::events::data::events::repo_account::RepoAccountRole",
            "e7eda913091faa78cd368f581aebee1c",
            "e7eda913091faa78cd368f581aebee1c",
        )
    },
    || {
        check::<super::repo_account::RepoAccountInitiated>(
            "iroha_data_model::events::data::events::repo_account::model::RepoAccountInitiated",
            "770ac42725a67a0d449a53984b823610",
            "770ac42725a67a0d449a53984b823610",
        )
    },
    || {
        check::<super::repo_account::RepoAccountSettled>(
            "iroha_data_model::events::data::events::repo_account::model::RepoAccountSettled",
            "0551acde8e0be9b0b83f2f7c9ecc8321",
            "0551acde8e0be9b0b83f2f7c9ecc8321",
        )
    },
    || {
        check::<super::repo_account::RepoAccountMarginCalled>(
            "iroha_data_model::events::data::events::repo_account::model::RepoAccountMarginCalled",
            "9d450fb7f34bc6383a2da5cc6b2935cb",
            "9d450fb7f34bc6383a2da5cc6b2935cb",
        )
    },
    || {
        check::<super::domain::DomainOwnerChanged>(
            "iroha_data_model::events::data::events::domain::model::DomainOwnerChanged",
            "5d2f038b1da2302b936343e71e93451a",
            "5d2f038b1da2302b936343e71e93451a",
        )
    },
    || {
        check::<super::domain::ScopedAccount>(
            "iroha_data_model::events::data::events::domain::model::ScopedAccount",
            "6265d0beff6e56be953937db99438876",
            "6265d0beff6e56be953937db99438876",
        )
    },
    || {
        check::<super::domain::ScopedAsset>(
            "iroha_data_model::events::data::events::domain::model::ScopedAsset",
            "cf117b4b4a6f6163e1b982dccbc16ea5",
            "cf117b4b4a6f6163e1b982dccbc16ea5",
        )
    },
    || {
        check::<super::domain::ScopedAssetDefinition>(
            "iroha_data_model::events::data::events::domain::model::ScopedAssetDefinition",
            "62e56d5ab376151af6fba6b1152e32e2",
            "62e56d5ab376151af6fba6b1152e32e2",
        )
    },
    || {
        check::<super::domain::AccountDomainLinkChanged>(
            "iroha_data_model::events::data::events::domain::model::AccountDomainLinkChanged",
            "8f2f34664150ccd4f00294f064ca4df8",
            "8f2f34664150ccd4f00294f064ca4df8",
        )
    },
    || {
        check::<super::domain::KaigiRosterSummary>(
            "iroha_data_model::events::data::events::domain::model::KaigiRosterSummary",
            "ab7003c220b8fcaeac7eadbc71e0ef55",
            "ab7003c220b8fcaeac7eadbc71e0ef55",
        )
    },
    || {
        check::<super::domain::KaigiRelayRegistrationSummary>(
            "iroha_data_model::events::data::events::domain::model::KaigiRelayRegistrationSummary",
            "096dcd90da7b9f60e52c6e20badc38b7",
            "096dcd90da7b9f60e52c6e20badc38b7",
        )
    },
    || {
        check::<super::domain::KaigiRelayUnregistrationSummary>(
            "iroha_data_model::events::data::events::domain::model::KaigiRelayUnregistrationSummary",
            "bed318e7b4bac439ae4db1c4463db254",
            "bed318e7b4bac439ae4db1c4463db254",
        )
    },
    || {
        check::<super::domain::KaigiStatusSummary>(
            "iroha_data_model::events::data::events::domain::model::KaigiStatusSummary",
            "713f0b30451daa2b5f7eb7ff5ff1fb11",
            "713f0b30451daa2b5f7eb7ff5ff1fb11",
        )
    },
    || {
        check::<super::domain::KaigiRelayManifestSummary>(
            "iroha_data_model::events::data::events::domain::model::KaigiRelayManifestSummary",
            "8e4732b261fc996f966403ae388fef32",
            "8e4732b261fc996f966403ae388fef32",
        )
    },
    || {
        check::<super::domain::KaigiRelayHealthSummary>(
            "iroha_data_model::events::data::events::domain::model::KaigiRelayHealthSummary",
            "6b82ee73b73664067b0304c08038655f",
            "6b82ee73b73664067b0304c08038655f",
        )
    },
    || {
        check::<super::domain::KaigiUsageSummary>(
            "iroha_data_model::events::data::events::domain::model::KaigiUsageSummary",
            "4d8ffa6a308a46f3af22a8b272fe9c42",
            "4d8ffa6a308a46f3af22a8b272fe9c42",
        )
    },
    || {
        check::<super::domain::StreamingPrivacyRelay>(
            "iroha_data_model::events::data::events::domain::model::StreamingPrivacyRelay",
            "10ae2766ee4fb79a141260d2d210234e",
            "10ae2766ee4fb79a141260d2d210234e",
        )
    },
    || {
        check::<super::domain::StreamingSoranetAccessKind>(
            "iroha_data_model::events::data::events::domain::model::StreamingSoranetAccessKind",
            "9d4375477fb54d108e577f9a9c516ce3",
            "9d4375477fb54d108e577f9a9c516ce3",
        )
    },
    || {
        check::<super::domain::StreamingSoranetStreamTag>(
            "iroha_data_model::events::data::events::domain::model::StreamingSoranetStreamTag",
            "3f57cb87c4c59d1986357e60e0831f99",
            "3f57cb87c4c59d1986357e60e0831f99",
        )
    },
    || {
        check::<super::domain::StreamingSoranetRoute>(
            "iroha_data_model::events::data::events::domain::model::StreamingSoranetRoute",
            "716111ba346bba464b80f0a0c7c724c9",
            "716111ba346bba464b80f0a0c7c724c9",
        )
    },
    || {
        check::<super::domain::StreamingPrivacyRoute>(
            "iroha_data_model::events::data::events::domain::model::StreamingPrivacyRoute",
            "f334001b3f54d15f2bbda74ad829fe3c",
            "f334001b3f54d15f2bbda74ad829fe3c",
        )
    },
    || {
        check::<super::domain::StreamingRouteBinding>(
            "iroha_data_model::events::data::events::domain::model::StreamingRouteBinding",
            "de14bdf1e7d79bbc4b12d6a18084331f",
            "de14bdf1e7d79bbc4b12d6a18084331f",
        )
    },
    || {
        check::<super::domain::StreamingTicketPolicy>(
            "iroha_data_model::events::data::events::domain::model::StreamingTicketPolicy",
            "4d7e04be79f3e95c3daed3e7055ae8d8",
            "4d7e04be79f3e95c3daed3e7055ae8d8",
        )
    },
    || {
        check::<super::domain::StreamingTicketCapabilities>(
            "iroha_data_model::events::data::events::domain::model::StreamingTicketCapabilities",
            "a8e9635fce58262e9328b67b1a0931c8",
            "a8e9635fce58262e9328b67b1a0931c8",
        )
    },
    || {
        check::<super::domain::StreamingTicketRecord>(
            "iroha_data_model::events::data::events::domain::model::StreamingTicketRecord",
            "b1399604c32efd93fd2222f6372c4d47",
            "b1399604c32efd93fd2222f6372c4d47",
        )
    },
    || {
        check::<super::domain::StreamingTicketReady>(
            "iroha_data_model::events::data::events::domain::model::StreamingTicketReady",
            "8b942cea019612707205d82ab410f3c4",
            "8b942cea019612707205d82ab410f3c4",
        )
    },
    || {
        check::<super::domain::StreamingTicketRevoked>(
            "iroha_data_model::events::data::events::domain::model::StreamingTicketRevoked",
            "c929654e53740858f25b79919bb64cc3",
            "c929654e53740858f25b79919bb64cc3",
        )
    },
    || {
        check::<super::trigger::TriggerNumberOfExecutionsChanged>(
            "iroha_data_model::events::data::events::trigger::model::TriggerNumberOfExecutionsChanged",
            "e38b877d49203af4cd287de19867cead",
            "e38b877d49203af4cd287de19867cead",
        )
    },
    || {
        check::<super::config::SccpRegistryOperation>(
            "iroha_data_model::events::data::events::config::model::SccpRegistryOperation",
            "da44fe5a5e1c894b567085c0d4311dde",
            "da44fe5a5e1c894b567085c0d4311dde",
        )
    },
    || {
        check::<super::config::SccpRegistryChanged>(
            "iroha_data_model::events::data::events::config::model::SccpRegistryChanged",
            "1590a4b8542039a7ddb9ae95d8498cb9",
            "1590a4b8542039a7ddb9ae95d8498cb9",
        )
    },
    || {
        check::<super::config::ParameterChanged>(
            "iroha_data_model::events::data::events::config::model::ParameterChanged",
            "91f3bf170dedca729b5150a6d1b57c25",
            "91f3bf170dedca729b5150a6d1b57c25",
        )
    },
    || {
        check::<super::config::ConfigurationEvent>(
            "iroha_data_model::events::data::events::config::model::ConfigurationEvent",
            "4175389872ff39d7c22fbde4d4e55eb7",
            "4175389872ff39d7c22fbde4d4e55eb7",
        )
    },
    || {
        check::<super::executor::ExecutorEvent>(
            "iroha_data_model::events::data::events::executor::model::ExecutorEvent",
            "0f0beb4ca91682ece5a090be222b10ee",
            "0f0beb4ca91682ece5a090be222b10ee",
        )
    },
    || {
        check::<super::executor::ExecutorUpgrade>(
            "iroha_data_model::events::data::events::executor::model::ExecutorUpgrade",
            "9f32a73058584ba22270581f62f08aaa",
            "9f32a73058584ba22270581f62f08aaa",
        )
    },
];

#[test]
fn captured_event_codec_schema_identities() {
    for check in CASES {
        check();
    }
}
