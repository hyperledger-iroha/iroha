//! Visitor that visits every node in Iroha syntax tree
#![allow(missing_docs, clippy::missing_errors_doc)]

use iroha_primitives::numeric::Numeric;

use crate::{
    isi::Log,
    prelude::*,
    query::{
        trigger::FindTriggers, AnyQueryBox, QueryWithFilter, QueryWithParams, SingularQueryBox,
    },
};

macro_rules! delegate {
    ( $($visitor:ident $(<$param:ident $(: $bound:path)?>)?($operation:ty)),+ $(,)? ) => { $(
        fn $visitor$(<$param $(: $bound)?>)?(&mut self, operation: $operation) {
            $visitor(self, operation);
        } )+
    };
}

/// Trait to validate Iroha entities.
/// Default implementation of non-leaf visitors runs `visit_` functions for leafs.
/// Default implementation for leaf visitors is blank.
///
/// This trait is based on the visitor pattern.
pub trait Visit {
    delegate! {
        // Visit SignedTransaction
        visit_transaction(&SignedTransaction),
        visit_instruction(&InstructionBox),
        visit_query(&AnyQueryBox),
        visit_singular_query(&SingularQueryBox),
        visit_iter_query(&QueryWithParams),

        // Visit InstructionBox
        visit_burn(&BurnBox),
        visit_grant(&GrantBox),
        visit_mint(&MintBox),
        visit_register(&RegisterBox),
        visit_remove_key_value(&RemoveKeyValueBox),
        visit_revoke(&RevokeBox),
        visit_set_key_value(&SetKeyValueBox),
        visit_transfer(&TransferBox),
        visit_unregister(&UnregisterBox),
        visit_upgrade(&Upgrade),

        visit_execute_trigger(&ExecuteTrigger),
        visit_execute_wasm(&ExecuteWasmBox),
        visit_set_parameter(&SetParameter),
        visit_log(&Log),
        visit_custom_instruction(&CustomInstruction),

        // Visit SingularQueryBox
        visit_find_executor_data_model(&FindExecutorDataModel),
        visit_find_parameters(&FindParameters),

        // Visit IterableQueryBox
        visit_find_domains(&QueryWithFilter<FindDomains>),
        visit_find_accounts(&QueryWithFilter<FindAccounts>),
        visit_find_assets(&QueryWithFilter<FindAssets>),
        visit_find_assets_definitions(&QueryWithFilter<FindAssetsDefinitions>),
        visit_find_nfts(&QueryWithFilter<FindNfts>),
        visit_find_roles(&QueryWithFilter<FindRoles>),
        visit_find_role_ids(&QueryWithFilter<FindRoleIds>),
        visit_find_permissions_by_account_id(&QueryWithFilter<FindPermissionsByAccountId>),
        visit_find_roles_by_account_id(&QueryWithFilter<FindRolesByAccountId>),
        visit_find_accounts_with_asset(&QueryWithFilter<FindAccountsWithAsset>),
        visit_find_peers(&QueryWithFilter<FindPeers>),
        visit_find_active_trigger_ids(&QueryWithFilter<FindActiveTriggerIds>),
        visit_find_triggers(&QueryWithFilter<FindTriggers>),
        visit_find_transactions(&QueryWithFilter<FindTransactions>),
        visit_find_blocks(&QueryWithFilter<FindBlocks>),
        visit_find_block_headers(&QueryWithFilter<FindBlockHeaders>),

        // Visit RegisterBox
        visit_register_peer(&Register<Peer>),
        visit_register_domain(&Register<Domain>),
        visit_register_account(&Register<Account>),
        visit_register_asset_definition(&Register<AssetDefinition>),
        visit_register_nft(&Register<Nft>),
        visit_register_role(&Register<Role>),
        visit_register_trigger(&Register<Trigger>),

        // Visit UnregisterBox
        visit_unregister_peer(&Unregister<Peer>),
        visit_unregister_domain(&Unregister<Domain>),
        visit_unregister_account(&Unregister<Account>),
        visit_unregister_asset_definition(&Unregister<AssetDefinition>),
        visit_unregister_nft(&Unregister<Nft>),
        // TODO: Need to allow role creator to unregister it somehow
        visit_unregister_role(&Unregister<Role>),
        visit_unregister_trigger(&Unregister<Trigger>),

        // Visit MintBox
        visit_mint_asset_numeric(&Mint<Numeric, Asset>),
        visit_mint_trigger_repetitions(&Mint<u32, Trigger>),

        // Visit BurnBox
        visit_burn_asset_numeric(&Burn<Numeric, Asset>),
        visit_burn_trigger_repetitions(&Burn<u32, Trigger>),

        // Visit TransferBox
        visit_transfer_asset_definition(&Transfer<Account, AssetDefinitionId, Account>),
        visit_transfer_asset_numeric(&Transfer<Asset, Numeric, Account>),
        visit_transfer_nft(&Transfer<Account, NftId, Account>),
        visit_transfer_domain(&Transfer<Account, DomainId, Account>),

        // Visit SetKeyValueBox
        visit_set_domain_key_value(&SetKeyValue<Domain>),
        visit_set_account_key_value(&SetKeyValue<Account>),
        visit_set_asset_definition_key_value(&SetKeyValue<AssetDefinition>),
        visit_set_nft_key_value(&SetKeyValue<Nft>),
        visit_set_trigger_key_value(&SetKeyValue<Trigger>),

        // Visit RemoveKeyValueBox
        visit_remove_domain_key_value(&RemoveKeyValue<Domain>),
        visit_remove_account_key_value(&RemoveKeyValue<Account>),
        visit_remove_asset_definition_key_value(&RemoveKeyValue<AssetDefinition>),
        visit_remove_nft_key_value(&RemoveKeyValue<Nft>),
        visit_remove_trigger_key_value(&RemoveKeyValue<Trigger>),

        // Visit GrantBox
        visit_grant_account_permission(&Grant<Permission, Account>),
        visit_grant_account_role(&Grant<RoleId, Account>),
        visit_grant_role_permission(&Grant<Permission, Role>),

        // Visit RevokeBox
        visit_revoke_account_permission(&Revoke<Permission, Account>),
        visit_revoke_account_role(&Revoke<RoleId, Account>),
        visit_revoke_role_permission(&Revoke<Permission, Role>),

        // Visit ExecuteWasmBox
        visit_execute_wasm_smartcontract(&WasmExecutable<WasmSmartContract>),
        visit_execute_wasm_trigger(&WasmExecutable<TriggerModule>),
    }
}

pub fn visit_transaction<V: Visit + ?Sized>(visitor: &mut V, transaction: &SignedTransaction) {
    match transaction.instructions() {
        Executable::Instructions(instructions) => {
            for isi in instructions {
                visitor.visit_instruction(isi);
            }
        }
    }
}

pub fn visit_singular_query<V: Visit + ?Sized>(visitor: &mut V, query: &SingularQueryBox) {
    macro_rules! singular_query_visitors {
        ( $($visitor:ident($query:ident)),+ $(,)? ) => {
            match query { $(
                SingularQueryBox::$query(query) => visitor.$visitor(&query), )+
            }
        };
    }

    singular_query_visitors! {
        visit_find_executor_data_model(FindExecutorDataModel),
        visit_find_parameters(FindParameters),
    }
}

pub fn visit_iter_query<V: Visit + ?Sized>(visitor: &mut V, query: &QueryWithParams) {
    macro_rules! iterable_query_visitors {
        ( $($visitor:ident($query:ident)),+ $(,)? ) => {
            match &query.query { $(
                QueryBox::$query(query) => visitor.$visitor(&query), )+
            }
        };
    }

    iterable_query_visitors! {
        visit_find_domains(FindDomains),
        visit_find_accounts(FindAccounts),
        visit_find_assets(FindAssets),
        visit_find_assets_definitions(FindAssetsDefinitions),
        visit_find_nfts(FindNfts),
        visit_find_roles(FindRoles),
        visit_find_role_ids(FindRoleIds),
        visit_find_permissions_by_account_id(FindPermissionsByAccountId),
        visit_find_roles_by_account_id(FindRolesByAccountId),
        visit_find_accounts_with_asset(FindAccountsWithAsset),
        visit_find_peers(FindPeers),
        visit_find_active_trigger_ids(FindActiveTriggerIds),
        visit_find_triggers(FindTriggers),
        visit_find_transactions(FindTransactions),
        visit_find_block_headers(FindBlockHeaders),
        visit_find_blocks(FindBlocks),
    }
}

pub fn visit_query<V: Visit + ?Sized>(visitor: &mut V, query: &AnyQueryBox) {
    match query {
        AnyQueryBox::Singular(query) => visitor.visit_singular_query(query),
        AnyQueryBox::Iterable(query) => visitor.visit_iter_query(query),
    }
}

/// Default validation for [`InstructionBox`].
///
/// # Warning
///
/// Instruction is executed following successful validation
pub fn visit_instruction<V: Visit + ?Sized>(visitor: &mut V, isi: &InstructionBox) {
    match isi {
        InstructionBox::SetParameter(variant_value) => visitor.visit_set_parameter(variant_value),
        InstructionBox::ExecuteTrigger(variant_value) => {
            visitor.visit_execute_trigger(variant_value)
        }
        InstructionBox::Log(variant_value) => visitor.visit_log(variant_value),
        InstructionBox::Burn(variant_value) => visitor.visit_burn(variant_value),
        InstructionBox::Grant(variant_value) => visitor.visit_grant(variant_value),
        InstructionBox::Mint(variant_value) => visitor.visit_mint(variant_value),
        InstructionBox::Register(variant_value) => visitor.visit_register(variant_value),
        InstructionBox::RemoveKeyValue(variant_value) => {
            visitor.visit_remove_key_value(variant_value)
        }
        InstructionBox::Revoke(variant_value) => visitor.visit_revoke(variant_value),
        InstructionBox::SetKeyValue(variant_value) => visitor.visit_set_key_value(variant_value),
        InstructionBox::Transfer(variant_value) => visitor.visit_transfer(variant_value),
        InstructionBox::Unregister(variant_value) => visitor.visit_unregister(variant_value),
        InstructionBox::Upgrade(variant_value) => visitor.visit_upgrade(variant_value),
        InstructionBox::Custom(custom) => visitor.visit_custom_instruction(custom),
        InstructionBox::ExecuteWasm(variant_value) => visitor.visit_execute_wasm(variant_value),
    }
}

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

pub fn visit_execute_wasm<V: Visit + ?Sized>(visitor: &mut V, isi: &ExecuteWasmBox) {
    match isi {
        ExecuteWasmBox::Smartcontract(obj) => visitor.visit_execute_wasm_smartcontract(obj),
        ExecuteWasmBox::Trigger(obj) => visitor.visit_execute_wasm_trigger(obj),
    }
}

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

pub fn visit_mint<V: Visit + ?Sized>(visitor: &mut V, isi: &MintBox) {
    match isi {
        MintBox::Asset(obj) => visitor.visit_mint_asset_numeric(obj),
        MintBox::TriggerRepetitions(obj) => visitor.visit_mint_trigger_repetitions(obj),
    }
}

pub fn visit_burn<V: Visit + ?Sized>(visitor: &mut V, isi: &BurnBox) {
    match isi {
        BurnBox::Asset(obj) => visitor.visit_burn_asset_numeric(obj),
        BurnBox::TriggerRepetitions(obj) => visitor.visit_burn_trigger_repetitions(obj),
    }
}

pub fn visit_transfer<V: Visit + ?Sized>(visitor: &mut V, isi: &TransferBox) {
    match isi {
        TransferBox::Domain(obj) => visitor.visit_transfer_domain(obj),
        TransferBox::AssetDefinition(obj) => visitor.visit_transfer_asset_definition(obj),
        TransferBox::Asset(obj) => visitor.visit_transfer_asset_numeric(obj),
        TransferBox::Nft(obj) => visitor.visit_transfer_nft(obj),
    }
}

pub fn visit_set_key_value<V: Visit + ?Sized>(visitor: &mut V, isi: &SetKeyValueBox) {
    match isi {
        SetKeyValueBox::Domain(obj) => visitor.visit_set_domain_key_value(obj),
        SetKeyValueBox::Account(obj) => visitor.visit_set_account_key_value(obj),
        SetKeyValueBox::AssetDefinition(obj) => visitor.visit_set_asset_definition_key_value(obj),
        SetKeyValueBox::Nft(obj) => visitor.visit_set_nft_key_value(obj),
        SetKeyValueBox::Trigger(obj) => visitor.visit_set_trigger_key_value(obj),
    }
}

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

pub fn visit_grant<V: Visit + ?Sized>(visitor: &mut V, isi: &GrantBox) {
    match isi {
        GrantBox::Permission(obj) => visitor.visit_grant_account_permission(obj),
        GrantBox::Role(obj) => visitor.visit_grant_account_role(obj),
        GrantBox::RolePermission(obj) => visitor.visit_grant_role_permission(obj),
    }
}

pub fn visit_revoke<V: Visit + ?Sized>(visitor: &mut V, isi: &RevokeBox) {
    match isi {
        RevokeBox::Permission(obj) => visitor.visit_revoke_account_permission(obj),
        RevokeBox::Role(obj) => visitor.visit_revoke_account_role(obj),
        RevokeBox::RolePermission(obj) => visitor.visit_revoke_role_permission(obj),
    }
}

macro_rules! leaf_visitors {
    ( $($visitor:ident($operation:ty)),+ $(,)? ) => { $(
        pub fn $visitor<V: Visit + ?Sized>(_visitor: &mut V, _operation: $operation) {

        } )+
    };
}

leaf_visitors! {
    // Instruction visitors
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
    visit_register_peer(&Register<Peer>),
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
    visit_execute_wasm_smartcontract(&WasmExecutable<WasmSmartContract>),
    visit_execute_wasm_trigger(&WasmExecutable<TriggerModule>),
    visit_log(&Log),
    visit_custom_instruction(&CustomInstruction),

    // Singular Query visitors
    visit_find_executor_data_model(&FindExecutorDataModel),
    visit_find_parameters(&FindParameters),

    // Iterable Query visitors
    visit_find_domains(&QueryWithFilter<FindDomains>),
    visit_find_accounts(&QueryWithFilter<FindAccounts>),
    visit_find_assets(&QueryWithFilter<FindAssets>),
    visit_find_assets_definitions(&QueryWithFilter<FindAssetsDefinitions>),
    visit_find_nfts(&QueryWithFilter<FindNfts>),
    visit_find_roles(&QueryWithFilter<FindRoles>),
    visit_find_role_ids(&QueryWithFilter<FindRoleIds>),
    visit_find_permissions_by_account_id(&QueryWithFilter<FindPermissionsByAccountId>),
    visit_find_roles_by_account_id(&QueryWithFilter<FindRolesByAccountId>),
    visit_find_accounts_with_asset(&QueryWithFilter<FindAccountsWithAsset>),
    visit_find_peers(&QueryWithFilter<FindPeers>),
    visit_find_active_trigger_ids(&QueryWithFilter<FindActiveTriggerIds>),
    visit_find_triggers(&QueryWithFilter<FindTriggers>),
    visit_find_transactions(&QueryWithFilter<FindTransactions>),
    visit_find_blocks(&QueryWithFilter<FindBlocks>),
    visit_find_block_headers(&QueryWithFilter<FindBlockHeaders>),
}
