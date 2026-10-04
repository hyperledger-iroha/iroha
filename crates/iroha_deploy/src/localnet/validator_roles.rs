//! Public role inventory for a generated canonical Taira parent, separate from lane authority.

use super::*;

/// Canonical generated public role inventory path, outside the active lane registry.
pub const TAIRA_VALIDATOR_ROLES_FILE: &str = "validator-roles.json";
const SCHEMA: &str = "iroha.taira.validator-role-inventory.v1";

/// One ordered generated validator role and its public identities.
#[derive(Debug, Clone, norito::derive::JsonSerialize, norito::derive::JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub struct TairaValidatorRoleV1 {
    /// Zero-based generated peer slot; not a physical lane identifier.
    pub index: u16,
    /// Canonical runtime signer account for this slot.
    pub validator: String,
    /// Canonical P2P identity for this slot.
    pub peer_id: String,
}

/// Public generated role metadata; this does not authorize any lane or deployment.
#[derive(Debug, Clone, norito::derive::JsonSerialize, norito::derive::JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub struct TairaValidatorRoleInventoryV1 {
    /// Exact first-release inventory schema.
    pub schema: String,
    /// Canonical Taira chain identifier.
    pub chain: String,
    /// Four generated validator roles, in peer-slot order.
    pub validators: Vec<TairaValidatorRoleV1>,
    /// Exact global committee quorum.
    pub quorum: u32,
}

impl TairaValidatorRoleInventoryV1 {
    /// Validate exact shape, ordering, canonical identities and distinctness.
    /// Consumers must additionally bind each row to its selected native peer configuration.
    pub fn validate(&self) -> Result<()> {
        let _guard = ChainDiscriminantGuard::enter(369);
        ensure!(
            self.schema == SCHEMA
                && self.chain == PUBLIC_TAIRA_CHAIN_ID
                && self.quorum == 3
                && self.validators.len() == usize::from(TAIRA_TESTNET_PEERS),
            "generated role inventory is not the canonical four-validator Taira inventory"
        );
        let mut accounts = BTreeSet::new();
        let mut peers = BTreeSet::new();
        for (index, row) in self.validators.iter().enumerate() {
            ensure!(
                usize::from(row.index) == index,
                "generated validator role order differs"
            );
            let account = AccountId::parse_encoded(&row.validator)
                .map_err(|_| eyre!("generated validator role account is invalid"))?;
            let peer: PeerId = row
                .peer_id
                .parse()
                .map_err(|_| eyre!("generated validator role peer identity is invalid"))?;
            ensure!(
                account.to_string() == row.validator
                    && peer.to_string() == row.peer_id
                    && accounts.insert(account)
                    && peers.insert(peer),
                "generated validator role identities are not canonical and distinct"
            );
        }
        Ok(())
    }
}

pub(super) fn write_taira_validator_roles(
    out_dir: &Path,
    peers: &[Peer],
    chain_discriminant: Option<u16>,
) -> Result<()> {
    let validators = peers
        .iter()
        .enumerate()
        .map(|(index, peer)| {
            Ok(TairaValidatorRoleV1 {
                index: u16::try_from(index)
                    .map_err(|_| eyre!("validator role index exceeds u16"))?,
                validator: account_id_runtime_literal(
                    &peer.validator_account_id(true),
                    chain_discriminant,
                ),
                peer_id: PeerId::from(peer.public_key.clone()).to_string(),
            })
        })
        .collect::<Result<Vec<_>>>()?;
    let inventory = TairaValidatorRoleInventoryV1 {
        schema: SCHEMA.to_owned(),
        chain: PUBLIC_TAIRA_CHAIN_ID.to_owned(),
        validators,
        quorum: 3,
    };
    inventory.validate()?;
    let raw = norito::json::to_json_pretty(&inventory)
        .wrap_err("serialize public Taira validator roles")?;
    custody::write(&out_dir.join(TAIRA_VALIDATOR_ROLES_FILE), raw)
        .wrap_err("write public Taira validator roles")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> TairaValidatorRoleInventoryV1 {
        let _guard = ChainDiscriminantGuard::enter(369);
        let peers = build_peers(4, Some(b"public-role-inventory-fixture"), 8080, 1337).unwrap();
        TairaValidatorRoleInventoryV1 {
            schema: SCHEMA.to_owned(),
            chain: PUBLIC_TAIRA_CHAIN_ID.to_owned(),
            validators: peers
                .iter()
                .enumerate()
                .map(|(index, peer)| TairaValidatorRoleV1 {
                    index: u16::try_from(index).unwrap(),
                    validator: peer.validator_account_id(true).to_string(),
                    peer_id: PeerId::from(peer.public_key.clone()).to_string(),
                })
                .collect(),
            quorum: 3,
        }
    }

    #[test]
    fn typed_inventory_rejects_lane_authority_fields_and_wrong_roles() {
        let _guard = ChainDiscriminantGuard::enter(369);
        let inventory = fixture();
        inventory.validate().unwrap();
        let raw = norito::json::to_json(&inventory).unwrap();
        let decoded: TairaValidatorRoleInventoryV1 = norito::json::from_str(&raw).unwrap();
        decoded.validate().unwrap();
        let mut wrong = inventory.clone();
        wrong.validators.swap(0, 1);
        assert!(wrong.validate().is_err());
        wrong = inventory.clone();
        wrong.validators[1].validator = wrong.validators[0].validator.clone();
        assert!(wrong.validate().is_err());
        wrong = inventory.clone();
        wrong.validators[1].peer_id = wrong.validators[0].peer_id.clone();
        assert!(wrong.validate().is_err());
        wrong = inventory.clone();
        wrong.quorum = 4;
        assert!(wrong.validate().is_err());
        let unknown = raw.replacen("{", "{\"lane\":\"is\",", 1);
        assert!(norito::json::from_str::<TairaValidatorRoleInventoryV1>(&unknown).is_err());
    }

    #[test]
    fn role_writer_does_not_materialize_an_active_lane_manifest() {
        let _guard = ChainDiscriminantGuard::enter(369);
        let temp = localnet_test_helpers::private_tempdir().unwrap();
        let peers = build_peers(4, Some(b"public-role-writer-fixture"), 8080, 1337).unwrap();
        write_taira_validator_roles(temp.path(), &peers, Some(369)).unwrap();
        assert!(!temp.path().join("lane-manifests").exists());
        let raw = fs::read(temp.path().join(TAIRA_VALIDATOR_ROLES_FILE)).unwrap();
        let inventory: TairaValidatorRoleInventoryV1 = norito::json::from_slice(&raw).unwrap();
        inventory.validate().unwrap();
    }
}
