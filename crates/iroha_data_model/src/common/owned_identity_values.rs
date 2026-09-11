//! Deterministic NFT and RWA storage values for exact identity and frame checks.

use crate::{
    nft::NftData,
    prelude::{AccountId, Hash, Json, NumericSpec, PublicKey, Quantity, RoleId},
    rwa::{RwaControlPolicy, RwaData, RwaId, RwaParentRef},
};
use iroha_model_base::domain::DomainId;
use iroha_model_base::metadata::Metadata;
use iroha_model_base::name::Name;

const OWNER_KEY: &str = "ed012004FF5B81046DDCCF19E2E451C45DFB6F53759D4EB30FA2EFA807284D1CC33016";
const CONTROLLER_KEY: &str =
    "ed01200376E59E9078B647F55003896B59758B7BE99908535EC24BAF80A6D52C8B3EB8";
const SECOND_CONTROLLER_KEY: &str =
    "ed0120BDF918243253B1E731FA096194C8928DA37C4D3226F97EEBD18CF5523D758D6C";

fn account(encoded_key: &str) -> AccountId {
    AccountId::new(
        encoded_key
            .parse::<PublicKey>()
            .expect("checked public fixture key"),
    )
}

fn populated_metadata() -> Metadata {
    let mut metadata = Metadata::default();
    metadata.insert(
        "archive_reference".parse().expect("metadata key"),
        Json::new("ipfs://bafy-owned-storage-identity"),
    );
    metadata.insert(
        "independently_verified".parse().expect("metadata key"),
        Json::new(true),
    );
    metadata
}

/// Deterministic NFT storage values for pre-declaration wire capture.
pub(super) fn nft_values() -> Vec<NftData> {
    let mut content = populated_metadata();
    content.insert(
        "display_name".parse().expect("NFT metadata key"),
        Json::new("Owned identity fixture NFT"),
    );
    vec![NftData {
        content,
        owned_by: account(OWNER_KEY),
    }]
}

/// Deterministic RWA storage values covering populated and absent optional state.
pub(super) fn rwa_values() -> Vec<RwaData> {
    let domain = DomainId::try_new("warehouse", "universal").expect("fixture RWA domain");
    let controls = RwaControlPolicy {
        controller_accounts: vec![account(CONTROLLER_KEY), account(SECOND_CONTROLLER_KEY)],
        controller_roles: vec![
            RoleId::new("custodian".parse::<Name>().expect("controller role")),
            RoleId::new(
                "compliance_officer"
                    .parse::<Name>()
                    .expect("controller role"),
            ),
        ],
        freeze_enabled: true,
        hold_enabled: false,
        force_transfer_enabled: true,
        redeem_enabled: false,
    };
    let populated = RwaData {
        quantity: "125.50".parse::<Quantity>().expect("fixture quantity"),
        spec: NumericSpec::fractional(2),
        primary_reference: "urn:iroha:rwa:warehouse-lot-7".to_owned(),
        status: Some("under_custody".parse::<Name>().expect("fixture status")),
        metadata: populated_metadata(),
        parents: vec![
            RwaParentRef::new(
                RwaId::generated(domain.clone(), Hash::new(b"owned-rwa-parent-alpha")),
                "25.00".parse::<Quantity>().expect("parent quantity"),
            ),
            RwaParentRef::new(
                RwaId::generated(domain, Hash::new(b"owned-rwa-parent-beta")),
                "12.50".parse::<Quantity>().expect("parent quantity"),
            ),
        ],
        controls,
        owned_by: account(OWNER_KEY),
        is_frozen: true,
        held_quantity: "5.25".parse::<Quantity>().expect("held quantity"),
    };
    let sparse = RwaData {
        quantity: Quantity::from(7_u64),
        spec: NumericSpec::integer(),
        primary_reference: "urn:iroha:rwa:unencumbered-lot".to_owned(),
        status: None,
        metadata: Metadata::default(),
        parents: Vec::new(),
        controls: RwaControlPolicy::default(),
        owned_by: account(CONTROLLER_KEY),
        is_frozen: false,
        held_quantity: Quantity::zero(),
    };
    vec![populated, sparse]
}
