//! Enable private Kaigi calls on generated developer localnets.
//!
//! A private call needs both verifying keys registered and selected by committed
//! chain policy, and a host allowed to write its domain's metadata. Generated
//! developer networks include them, so private calls work without a governance
//! step. Public network profiles keep their own decision.

use color_eyre::eyre::{Result, eyre};
use iroha_core_zk::kaigi_client_v1::kaigi_verifying_key_registrations_v1;
use iroha_data_model::{
    account::AccountId,
    isi::{Grant, verifying_keys::RegisterVerifyingKey},
    parameter::Parameter,
    proof::VerifyingKeyId,
    zk_validity_policy::{ZkValidityPolicyV1, ZkVerifyingKeyRefV1},
};
use iroha_executor_data_model::permission::domain::CanModifyDomainMetadata;
use iroha_genesis::RawGenesisTransaction;
use iroha_model_base::domain::DomainId;

fn policy_reference(id: &VerifyingKeyId) -> ZkVerifyingKeyRefV1 {
    ZkVerifyingKeyRefV1 {
        backend: id.backend.clone(),
        name: id.name.clone(),
    }
}

/// Register both Kaigi verifying keys, select them in the committed ZK policy and let
/// the client host calls in the universal domain.
pub(super) fn append_private_calls(
    genesis: RawGenesisTransaction,
    host: &AccountId,
) -> Result<RawGenesisTransaction> {
    let [authorization, usage] =
        kaigi_verifying_key_registrations_v1().map_err(|error| eyre!(error.to_string()))?;
    let mut policy = ZkValidityPolicyV1::protocol_default();
    policy.proofs.kaigi_authorization_vk = Some(policy_reference(&authorization.0));
    policy.proofs.kaigi_usage_vk = Some(policy_reference(&usage.0));
    let mut builder = genesis
        .into_builder()
        .next_transaction()
        .append_parameter(Parameter::Custom(policy.into_custom_parameter()))
        .append_instruction(Grant::account_permission(
            CanModifyDomainMetadata {
                domain: DomainId::parse_fully_qualified(super::genesis::UNIVERSAL_DOMAIN)?,
            },
            host.clone(),
        ));
    for (id, record) in [authorization, usage] {
        builder = builder.append_instruction(RegisterVerifyingKey { id, record });
    }
    builder.build_raw()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::localnet::{
        generate_localnet,
        genesis::UNIVERSAL_DOMAIN,
        tests::{assert_genesis_applies, options, private_parent},
    };
    use iroha_core_zk::kaigi_client_v1::{KAIGI_AUTHORIZATION_VK_NAME_V1, KAIGI_USAGE_VK_NAME_V1};
    use iroha_data_model::{
        isi::GrantBox, parameter::system::SumeragiConsensusMode, prelude::Permission,
    };
    use std::{fs, io::BufWriter};

    #[test]
    fn generated_developer_localnet_enables_private_calls() {
        let (_temporary, parent) = private_parent();
        let out_dir = parent.join("localnet");
        let opts = options(out_dir.clone(), SumeragiConsensusMode::Permissioned);
        generate_localnet(&opts, &mut BufWriter::new(Vec::new())).expect("generate localnet");
        assert_genesis_applies(&out_dir);
        let manifest =
            RawGenesisTransaction::from_path(out_dir.join("genesis.json")).expect("parse genesis");

        let registered = manifest
            .instructions()
            .filter_map(|instruction| instruction.as_any().downcast_ref::<RegisterVerifyingKey>())
            .collect::<Vec<_>>();
        for name in [KAIGI_AUTHORIZATION_VK_NAME_V1, KAIGI_USAGE_VK_NAME_V1] {
            assert!(
                registered.iter().any(|register| register.id.name == name
                    && register.record.is_active()
                    && register.record.key.is_some()),
                "localnet genesis must register an active {name} verifying key"
            );
        }

        let client_config = fs::read_to_string(out_dir.join("client.toml"))
            .expect("read client config")
            .parse::<toml::Table>()
            .expect("parse client config");
        let client = AccountId::new(
            client_config["account"]["public_key"]
                .as_str()
                .expect("client public key")
                .parse()
                .expect("parse client public key"),
        );
        let host_permission = Permission::from(CanModifyDomainMetadata {
            domain: DomainId::parse_fully_qualified(UNIVERSAL_DOMAIN).expect("universal domain"),
        });
        assert!(
            manifest.instructions().any(|instruction| {
                let Some(GrantBox::Permission(grant)) =
                    instruction.as_any().downcast_ref::<GrantBox>()
                else {
                    return false;
                };
                grant.destination() == &client && grant.object() == &host_permission
            }),
            "the localnet client must be able to host calls in the universal domain"
        );

        let parameters = manifest.effective_parameters().expect("parameters");
        let policy = parameters
            .custom()
            .get(&ZkValidityPolicyV1::parameter_id())
            .and_then(ZkValidityPolicyV1::from_custom_parameter)
            .expect("localnet genesis selects a ZK policy")
            .expect("valid ZK policy");
        assert_eq!(
            policy
                .proofs
                .kaigi_authorization_vk
                .as_ref()
                .map(|key| key.name.as_str()),
            Some(KAIGI_AUTHORIZATION_VK_NAME_V1)
        );
        assert_eq!(
            policy
                .proofs
                .kaigi_usage_vk
                .as_ref()
                .map(|key| key.name.as_str()),
            Some(KAIGI_USAGE_VK_NAME_V1)
        );
    }
}
