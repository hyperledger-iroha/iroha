//! Visitor helper functions for instructions.

use iroha_primitives::numeric::Numeric;

use super::Visit;
use crate::{
    isi::{
        Instruction, Log, RegisterPeerWithPop,
        nexus::SetLaneRelayEmergencyValidators,
        staking::{
            ActivatePublicLaneValidator, ExitPublicLaneValidator, RegisterPublicLaneValidator,
        },
    },
    prelude::*,
};

/// Dispatch a boxed instruction to the corresponding visitor hook.
pub fn visit_instruction<V: Visit + ?Sized>(visitor: &mut V, isi: &InstructionBox) {
    if let Some(v) = isi.as_any().downcast_ref::<SetParameter>() {
        visitor.visit_set_parameter(v);
    } else if let Some(v) = isi.as_any().downcast_ref::<ExecuteTrigger>() {
        visitor.visit_execute_trigger(v);
    } else if let Some(v) = isi.as_any().downcast_ref::<Log>() {
        visitor.visit_log(v);
    } else if let Some(v) = isi.as_any().downcast_ref::<BurnBox>() {
        visitor.visit_burn(v);
    } else if let Some(v) = isi.as_any().downcast_ref::<GrantBox>() {
        visitor.visit_grant(v);
    } else if let Some(v) = isi.as_any().downcast_ref::<MintBox>() {
        visitor.visit_mint(v);
    } else if let Some(v) = isi.as_any().downcast_ref::<RegisterBox>() {
        visitor.visit_register(v);
    } else if let Some(v) = isi.as_any().downcast_ref::<RemoveKeyValueBox>() {
        visitor.visit_remove_key_value(v);
    } else if let Some(v) = isi.as_any().downcast_ref::<RevokeBox>() {
        visitor.visit_revoke(v);
    } else if let Some(v) = isi.as_any().downcast_ref::<SetKeyValueBox>() {
        visitor.visit_set_key_value(v);
    } else if let Some(v) = isi.as_any().downcast_ref::<TransferBox>() {
        visitor.visit_transfer(v);
    } else if let Some(v) = isi.as_any().downcast_ref::<UnregisterBox>() {
        visitor.visit_unregister(v);
    } else if let Some(v) = isi.as_any().downcast_ref::<Upgrade>() {
        visitor.visit_upgrade(v);
    } else if let Some(v) = isi.as_any().downcast_ref::<CustomInstruction>() {
        visitor.visit_custom_instruction(v);
    } else if let Some(v) = isi.as_any().downcast_ref::<PublishPedersenParams>() {
        visitor.visit_publish_pedersen_params(v);
    } else if let Some(v) = isi.as_any().downcast_ref::<SetPedersenParamsLifecycle>() {
        visitor.visit_set_pedersen_params_lifecycle(v);
    } else if let Some(v) = isi.as_any().downcast_ref::<PublishPoseidonParams>() {
        visitor.visit_publish_poseidon_params(v);
    } else if let Some(v) = isi.as_any().downcast_ref::<SetPoseidonParamsLifecycle>() {
        visitor.visit_set_poseidon_params_lifecycle(v);
    } else if let Some(v) = isi.as_any().downcast_ref::<ClaimTwitterFollowReward>() {
        visitor.visit_claim_twitter_follow_reward(v);
    } else if let Some(v) = isi.as_any().downcast_ref::<SendToTwitter>() {
        visitor.visit_send_to_twitter(v);
    } else if let Some(v) = isi.as_any().downcast_ref::<CancelTwitterEscrow>() {
        visitor.visit_cancel_twitter_escrow(v);
    } else if let Some(v) = isi.as_any().downcast_ref::<RegisterPublicLaneValidator>() {
        visitor.visit_register_public_lane_validator(v);
    } else if let Some(v) = isi.as_any().downcast_ref::<ActivatePublicLaneValidator>() {
        visitor.visit_activate_public_lane_validator(v);
    } else if let Some(v) = isi.as_any().downcast_ref::<ExitPublicLaneValidator>() {
        visitor.visit_exit_public_lane_validator(v);
    } else if let Some(v) = isi
        .as_any()
        .downcast_ref::<SetLaneRelayEmergencyValidators>()
    {
        visitor.visit_set_lane_relay_emergency_validators(v);
    } else {
        unreachable!("Unknown instruction type");
    }
}

/// Dispatch register variants like peers, domains, and triggers.
pub fn visit_register<V: Visit + ?Sized>(visitor: &mut V, isi: &RegisterBox) {
    match isi {
        RegisterBox::Peer(obj) => visitor.visit_register_peer(obj),
        RegisterBox::Domain(obj) => visitor.visit_register_domain(obj),
        RegisterBox::Account(obj) => visitor.visit_register_account(obj),
        RegisterBox::AssetDefinition(obj) => visitor.visit_register_asset_definition(obj),
        RegisterBox::Nft(obj) => visitor.visit_register_nft(obj),
        RegisterBox::Role(obj) => visitor.visit_register_role(obj),
        RegisterBox::Trigger(obj) => visitor.visit_register_trigger(obj),
    }
}

/// Dispatch unregister variants across all registerable entities.
pub fn visit_unregister<V: Visit + ?Sized>(visitor: &mut V, isi: &UnregisterBox) {
    match isi {
        UnregisterBox::Peer(obj) => visitor.visit_unregister_peer(obj),
        UnregisterBox::Domain(obj) => visitor.visit_unregister_domain(obj),
        UnregisterBox::Account(obj) => visitor.visit_unregister_account(obj),
        UnregisterBox::AssetDefinition(obj) => visitor.visit_unregister_asset_definition(obj),
        UnregisterBox::Nft(obj) => visitor.visit_unregister_nft(obj),
        UnregisterBox::Role(obj) => visitor.visit_unregister_role(obj),
        UnregisterBox::Trigger(obj) => visitor.visit_unregister_trigger(obj),
    }
}

/// Dispatch mint variants to the appropriate hook.
pub fn visit_mint<V: Visit + ?Sized>(visitor: &mut V, isi: &MintBox) {
    match isi {
        MintBox::Asset(obj) => visitor.visit_mint_asset_numeric(obj),
        MintBox::TriggerRepetitions(obj) => visitor.visit_mint_trigger_repetitions(obj),
    }
}

/// Dispatch burn variants to the appropriate hook.
pub fn visit_burn<V: Visit + ?Sized>(visitor: &mut V, isi: &BurnBox) {
    match isi {
        BurnBox::Asset(obj) => visitor.visit_burn_asset_numeric(obj),
        BurnBox::TriggerRepetitions(obj) => visitor.visit_burn_trigger_repetitions(obj),
    }
}

/// Dispatch transfer variants to the appropriate hook.
pub fn visit_transfer<V: Visit + ?Sized>(visitor: &mut V, isi: &TransferBox) {
    match isi {
        TransferBox::Domain(obj) => visitor.visit_transfer_domain(obj),
        TransferBox::AssetDefinition(obj) => visitor.visit_transfer_asset_definition(obj),
        TransferBox::Asset(obj) => visitor.visit_transfer_asset_numeric(obj),
        TransferBox::Nft(obj) => visitor.visit_transfer_nft(obj),
    }
}

/// Dispatch set-key-value variants to the appropriate hook.
pub fn visit_set_key_value<V: Visit + ?Sized>(visitor: &mut V, isi: &SetKeyValueBox) {
    match isi {
        SetKeyValueBox::Domain(obj) => visitor.visit_set_domain_key_value(obj),
        SetKeyValueBox::Account(obj) => visitor.visit_set_account_key_value(obj),
        SetKeyValueBox::AssetDefinition(obj) => visitor.visit_set_asset_definition_key_value(obj),
        SetKeyValueBox::Nft(obj) => visitor.visit_set_nft_key_value(obj),
        SetKeyValueBox::Trigger(obj) => visitor.visit_set_trigger_key_value(obj),
    }
}

/// Dispatch remove-key-value variants to the appropriate hook.
pub fn visit_remove_key_value<V: Visit + ?Sized>(visitor: &mut V, isi: &RemoveKeyValueBox) {
    match isi {
        RemoveKeyValueBox::Domain(obj) => visitor.visit_remove_domain_key_value(obj),
        RemoveKeyValueBox::Account(obj) => visitor.visit_remove_account_key_value(obj),
        RemoveKeyValueBox::AssetDefinition(obj) => {
            visitor.visit_remove_asset_definition_key_value(obj)
        }
        RemoveKeyValueBox::Nft(obj) => visitor.visit_remove_nft_key_value(obj),
        RemoveKeyValueBox::Trigger(obj) => visitor.visit_remove_trigger_key_value(obj),
    }
}

/// Dispatch grant variants to the appropriate hook.
pub fn visit_grant<V: Visit + ?Sized>(visitor: &mut V, isi: &GrantBox) {
    match isi {
        GrantBox::Permission(obj) => visitor.visit_grant_account_permission(obj),
        GrantBox::Role(obj) => visitor.visit_grant_account_role(obj),
        GrantBox::RolePermission(obj) => visitor.visit_grant_role_permission(obj),
    }
}

/// Dispatch revoke variants to the appropriate hook.
pub fn visit_revoke<V: Visit + ?Sized>(visitor: &mut V, isi: &RevokeBox) {
    match isi {
        RevokeBox::Permission(obj) => visitor.visit_revoke_account_permission(obj),
        RevokeBox::Role(obj) => visitor.visit_revoke_account_role(obj),
        RevokeBox::RolePermission(obj) => visitor.visit_revoke_role_permission(obj),
    }
}

/// Macro generating visitor method signatures for every instruction variant.
#[macro_export]
macro_rules! instruction_visitors {
    ($macro:ident) => {
        $macro! {
            visit_register_account(&Register<Account>),
            visit_unregister_account(&Unregister<Account>),
            visit_set_account_key_value(&SetKeyValue<Account>),
            visit_remove_account_key_value(&RemoveKeyValue<Account>),
            visit_register_nft(&Register<Nft>),
            visit_unregister_nft(&Unregister<Nft>),
            visit_mint_asset_numeric(&Mint<Numeric, Asset>),
            visit_burn_asset_numeric(&Burn<Numeric, Asset>),
            visit_transfer_asset_numeric(&Transfer<Asset, Numeric, Account>),
            visit_transfer_nft(&Transfer<Account, NftId, Account>),
            visit_set_nft_key_value(&SetKeyValue<Nft>),
            visit_remove_nft_key_value(&RemoveKeyValue<Nft>),
            visit_set_trigger_key_value(&SetKeyValue<Trigger>),
            visit_remove_trigger_key_value(&RemoveKeyValue<Trigger>),
            visit_register_asset_definition(&Register<AssetDefinition>),
            visit_unregister_asset_definition(&Unregister<AssetDefinition>),
            visit_transfer_asset_definition(&Transfer<Account, AssetDefinitionId, Account>),
            visit_set_asset_definition_key_value(&SetKeyValue<AssetDefinition>),
            visit_remove_asset_definition_key_value(&RemoveKeyValue<AssetDefinition>),
            visit_register_domain(&Register<Domain>),
            visit_unregister_domain(&Unregister<Domain>),
            visit_transfer_domain(&Transfer<Account, DomainId, Account>),
            visit_set_domain_key_value(&SetKeyValue<Domain>),
            visit_remove_domain_key_value(&RemoveKeyValue<Domain>),
            visit_register_peer(&RegisterPeerWithPop),
            visit_unregister_peer(&Unregister<Peer>),
            visit_grant_account_permission(&Grant<Permission, Account>),
            visit_revoke_account_permission(&Revoke<Permission, Account>),
            visit_register_role(&Register<Role>),
            visit_unregister_role(&Unregister<Role>),
            visit_grant_account_role(&Grant<RoleId, Account>),
            visit_revoke_account_role(&Revoke<RoleId, Account>),
            visit_grant_role_permission(&Grant<Permission, Role>),
            visit_revoke_role_permission(&Revoke<Permission, Role>),
            visit_register_trigger(&Register<Trigger>),
            visit_unregister_trigger(&Unregister<Trigger>),
            visit_mint_trigger_repetitions(&Mint<u32, Trigger>),
            visit_burn_trigger_repetitions(&Burn<u32, Trigger>),
            visit_upgrade(&Upgrade),
            visit_set_parameter(&SetParameter),
            visit_execute_trigger(&ExecuteTrigger),
            visit_log(&Log),
            visit_custom_instruction(&CustomInstruction),
            visit_publish_pedersen_params(&PublishPedersenParams),
            visit_set_pedersen_params_lifecycle(&SetPedersenParamsLifecycle),
            visit_publish_poseidon_params(&PublishPoseidonParams),
            visit_set_poseidon_params_lifecycle(&SetPoseidonParamsLifecycle),
            visit_claim_twitter_follow_reward(&ClaimTwitterFollowReward),
            visit_send_to_twitter(&SendToTwitter),
            visit_cancel_twitter_escrow(&CancelTwitterEscrow),
            visit_register_public_lane_validator(&RegisterPublicLaneValidator),
            visit_activate_public_lane_validator(&ActivatePublicLaneValidator),
            visit_exit_public_lane_validator(&ExitPublicLaneValidator),
            visit_set_lane_relay_emergency_validators(&SetLaneRelayEmergencyValidators),
        }
    };
}

macro_rules! define_instruction_visitors {
    ( $( $visitor:ident($operation:ty) ),+ $(,)? ) => { $(
        #[doc = concat!("Visit ", stringify!($operation), ".")]
        pub fn $visitor<V: Visit + ?Sized>(_visitor: &mut V, _operation: $operation) {}
    )+ };
}

instruction_visitors!(define_instruction_visitors);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prelude::*;
    use iroha_crypto::{Algorithm, KeyPair};

    struct CountingVisitor {
        logs: usize,
    }

    impl Visit for CountingVisitor {
        fn visit_log(&mut self, _: &Log) {
            self.logs += 1;
        }
    }

    #[test]
    fn visit_log_dispatches() {
        let mut visitor = CountingVisitor { logs: 0 };
        let isi = InstructionBox::from(Log {
            level: Level::INFO,
            msg: "test".to_string(),
        });
        visit_instruction(&mut visitor, &isi);
        assert_eq!(visitor.logs, 1);
    }

    #[test]
    fn visit_register_public_lane_validator_dispatches() {
        struct RegisterVisitor {
            called: bool,
        }

        impl Visit for RegisterVisitor {
            fn visit_register_public_lane_validator(&mut self, _: &RegisterPublicLaneValidator) {
                self.called = true;
            }
        }

        let domain: DomainId = "wonderland".parse().expect("domain id");
        let key_pair = KeyPair::from_seed(vec![0x11; 32], Algorithm::Ed25519);
        let validator = AccountId::new(key_pair.public_key().clone());
        let instruction = RegisterPublicLaneValidator::new(
            LaneId::SINGLE,
            validator.clone(),
            validator,
            Numeric::from(1u64),
            Metadata::default(),
        );
        let isi = InstructionBox::from(instruction);

        let mut visitor = RegisterVisitor { called: false };
        visit_instruction(&mut visitor, &isi);
        assert!(visitor.called);
    }
}
