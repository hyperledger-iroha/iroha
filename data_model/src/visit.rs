//! Visitor that visits every node in Iroha syntax tree
#![allow(missing_docs, clippy::missing_errors_doc)]

#[cfg(not(feature = "std"))]
use alloc::format;

use iroha_crypto::PublicKey;

use crate::{evaluate::ExpressionEvaluator, isi::Log, prelude::*, NumericValue};

macro_rules! delegate {
    ( $($visitor:ident $(<$param:ident $(: $bound:path)?>)?($operation:ty)),+ $(,)? ) => { $(
        fn $visitor$(<$param $(: $bound)?>)?(&mut self, authority: &AccountId, operation: $operation) {
            $visitor(self, authority, operation);
        } )+
    };
}

macro_rules! evaluate_expr {
    ($visitor:ident, $authority:ident, <$isi:ident as $isi_type:ty>::$field:ident()) => {{
        $visitor.visit_expression($authority, $isi.$field());

        $visitor.evaluate($isi.$field()).expect(&format!(
            "Failed to evaluate field '{}::{}'",
            stringify!($isi_type),
            stringify!($field),
        ))
    }};
}

/// Trait to validate Iroha entities.
/// Default implementation of non-leaf visitors runs `visit_` functions for leafs.
/// Default implementation for leaf visitors is blank.
///
/// This trait is based on the visitor pattern.
pub trait Visit: ExpressionEvaluator {
    delegate! {
        visit_unsupported<T: core::fmt::Debug>(T),

        // Visit SignedTransaction
        visit_transaction(&SignedTransaction),
        visit_instruction(&InstructionExpr),
        visit_expression<V>(&EvaluatesTo<V>),
        visit_wasm(&WasmSmartContract),
        visit_query(&QueryBox),

        // Visit InstructionExpr
        visit_burn(&BurnExpr),
        visit_fail(&Fail),
        visit_grant(&GrantExpr),
        visit_if(&ConditionalExpr),
        visit_mint(&MintExpr),
        visit_pair(&PairExpr),
        visit_register(&RegisterExpr),
        visit_remove_key_value(&RemoveKeyValueExpr),
        visit_revoke(&RevokeExpr),
        visit_sequence(&SequenceExpr),
        visit_set_key_value(&SetKeyValueExpr),
        visit_transfer(&TransferExpr),
        visit_unregister(&UnregisterExpr),
        visit_upgrade(&UpgradeExpr),

        visit_execute_trigger(ExecuteTrigger),
        visit_new_parameter(NewParameter),
        visit_set_parameter(SetParameter),
        visit_log(Log),

        // Visit QueryBox
        visit_find_account_by_id(&FindAccountById),
        visit_find_account_key_value_by_id_and_key(&FindAccountKeyValueByIdAndKey),
        visit_find_accounts_by_domain_id(&FindAccountsByDomainId),
        visit_find_accounts_by_name(&FindAccountsByName),
        visit_find_accounts_with_asset(&FindAccountsWithAsset),
        visit_find_all_accounts(&FindAllAccounts),
        visit_find_all_active_trigger_ids(&FindAllActiveTriggerIds),
        visit_find_all_assets(&FindAllAssets),
        visit_find_all_assets_definitions(&FindAllAssetsDefinitions),
        visit_find_all_block_headers(&FindAllBlockHeaders),
        visit_find_all_blocks(&FindAllBlocks),
        visit_find_all_domains(&FindAllDomains),
        visit_find_all_parammeters(&FindAllParameters),
        visit_find_all_peers(&FindAllPeers),
        visit_find_permission_token_schema(&FindPermissionTokenSchema),
        visit_find_all_role_ids(&FindAllRoleIds),
        visit_find_all_roles(&FindAllRoles),
        visit_find_all_transactions(&FindAllTransactions),
        visit_find_asset_by_id(&FindAssetById),
        visit_find_asset_definition_by_id(&FindAssetDefinitionById),
        visit_find_asset_definition_key_value_by_id_and_key(&FindAssetDefinitionKeyValueByIdAndKey),
        visit_find_asset_key_value_by_id_and_key(&FindAssetKeyValueByIdAndKey),
        visit_find_asset_quantity_by_id(&FindAssetQuantityById),
        visit_find_assets_by_account_id(&FindAssetsByAccountId),
        visit_find_assets_by_asset_definition_id(&FindAssetsByAssetDefinitionId),
        visit_find_assets_by_domain_id(&FindAssetsByDomainId),
        visit_find_assets_by_domain_id_and_asset_definition_id(&FindAssetsByDomainIdAndAssetDefinitionId),
        visit_find_assets_by_name(&FindAssetsByName),
        visit_find_block_header_by_hash(&FindBlockHeaderByHash),
        visit_find_domain_by_id(&FindDomainById),
        visit_find_domain_key_value_by_id_and_key(&FindDomainKeyValueByIdAndKey),
        visit_find_permission_tokens_by_account_id(&FindPermissionTokensByAccountId),
        visit_find_role_by_role_id(&FindRoleByRoleId),
        visit_find_roles_by_account_id(&FindRolesByAccountId),
        visit_find_total_asset_quantity_by_asset_definition_id(&FindTotalAssetQuantityByAssetDefinitionId),
        visit_find_transaction_by_hash(&FindTransactionByHash),
        visit_find_transactions_by_account_id(&FindTransactionsByAccountId),
        visit_find_trigger_by_id(&FindTriggerById),
        visit_find_trigger_key_value_by_id_and_key(&FindTriggerKeyValueByIdAndKey),
        visit_find_triggers_by_domain_id(&FindTriggersByDomainId),

        // Visit RegisterExpr
        visit_register_peer(Register<Peer>),
        visit_register_domain(Register<Domain>),
        visit_register_account(Register<Account>),
        visit_register_asset_definition(Register<AssetDefinition>),
        visit_register_asset(Register<Asset>),
        visit_register_role(Register<Role>),
        visit_register_trigger(Register<Trigger<TriggeringFilterBox, Executable>>),

        // Visit UnregisterExpr
        visit_unregister_peer(Unregister<Peer>),
        visit_unregister_domain(Unregister<Domain>),
        visit_unregister_account(Unregister<Account>),
        visit_unregister_asset_definition(Unregister<AssetDefinition>),
        visit_unregister_asset(Unregister<Asset>),
        // TODO: Need to allow role creator to unregister it somehow
        visit_unregister_role(Unregister<Role>),
        visit_unregister_trigger(Unregister<Trigger<TriggeringFilterBox, Executable>>),

        // Visit MintExpr
        visit_mint_asset(Mint<NumericValue, Asset>),
        visit_mint_account_public_key(Mint<PublicKey, Account>),
        visit_mint_account_signature_check_condition(Mint<SignatureCheckCondition, Account>),
        visit_mint_trigger_repetitions(Mint<u32, Trigger<TriggeringFilterBox, Executable>>),

        // Visit BurnExpr
        visit_burn_account_public_key(Burn<PublicKey, Account>),
        visit_burn_asset(Burn<NumericValue, Asset>),
        visit_burn_trigger_repetitions(Burn<u32, Trigger<TriggeringFilterBox, Executable>>),

        // Visit TransferExpr
        visit_transfer_asset_definition(Transfer<Account, AssetDefinitionId, Account>),
        visit_transfer_asset(Transfer<Asset, NumericValue, Account>),
        visit_transfer_domain(Transfer<Account, DomainId, Account>),

        // Visit SetKeyValueExpr
        visit_set_domain_key_value(SetKeyValue<Domain>),
        visit_set_account_key_value(SetKeyValue<Account>),
        visit_set_asset_definition_key_value(SetKeyValue<AssetDefinition>),
        visit_set_asset_key_value(SetKeyValue<Asset>),

        // Visit RemoveKeyValueExpr
        visit_remove_domain_key_value(RemoveKeyValue<Domain>),
        visit_remove_account_key_value(RemoveKeyValue<Account>),
        visit_remove_asset_definition_key_value(RemoveKeyValue<AssetDefinition>),
        visit_remove_asset_key_value(RemoveKeyValue<Asset>),

        // Visit GrantExpr
        visit_grant_account_permission(Grant<PermissionToken>),
        visit_grant_account_role(Grant<RoleId>),

        // Visit RevokeExpr
        visit_revoke_account_permission(Revoke<PermissionToken>),
        visit_revoke_account_role(Revoke<RoleId>),

        // Visit UpgradeExpr
        visit_upgrade_executor(Upgrade<Executor>),
    }
}

/// Called when visiting any unsupported syntax tree node
fn visit_unsupported<V: Visit + ?Sized, T: core::fmt::Debug>(
    _visitor: &mut V,
    _authority: &AccountId,
    _isi: T,
) {
}

pub fn visit_transaction<V: Visit + ?Sized>(
    visitor: &mut V,
    authority: &AccountId,
    transaction: &SignedTransaction,
) {
    match transaction.payload().instructions() {
        Executable::Wasm(wasm) => visitor.visit_wasm(authority, wasm),
        Executable::Instructions(instructions) => {
            for isi in instructions {
                visitor.visit_instruction(authority, isi);
            }
        }
    }
}

/// Default validation for [`QueryBox`].
pub fn visit_query<V: Visit + ?Sized>(visitor: &mut V, authority: &AccountId, query: &QueryBox) {
    macro_rules! query_visitors {
        ( $($visitor:ident($query:ident)),+ $(,)? ) => {
            match query { $(
                QueryBox::$query(query) => visitor.$visitor(authority, &query), )+
            }
        };
    }

    query_visitors! {
        visit_find_account_by_id(FindAccountById),
        visit_find_account_key_value_by_id_and_key(FindAccountKeyValueByIdAndKey),
        visit_find_accounts_by_domain_id(FindAccountsByDomainId),
        visit_find_accounts_by_name(FindAccountsByName),
        visit_find_accounts_with_asset(FindAccountsWithAsset),
        visit_find_all_accounts(FindAllAccounts),
        visit_find_all_active_trigger_ids(FindAllActiveTriggerIds),
        visit_find_all_assets(FindAllAssets),
        visit_find_all_assets_definitions(FindAllAssetsDefinitions),
        visit_find_all_block_headers(FindAllBlockHeaders),
        visit_find_all_blocks(FindAllBlocks),
        visit_find_all_domains(FindAllDomains),
        visit_find_all_parammeters(FindAllParameters),
        visit_find_all_peers(FindAllPeers),
        visit_find_permission_token_schema(FindPermissionTokenSchema),
        visit_find_all_role_ids(FindAllRoleIds),
        visit_find_all_roles(FindAllRoles),
        visit_find_all_transactions(FindAllTransactions),
        visit_find_asset_by_id(FindAssetById),
        visit_find_asset_definition_by_id(FindAssetDefinitionById),
        visit_find_asset_definition_key_value_by_id_and_key(FindAssetDefinitionKeyValueByIdAndKey),
        visit_find_asset_key_value_by_id_and_key(FindAssetKeyValueByIdAndKey),
        visit_find_asset_quantity_by_id(FindAssetQuantityById),
        visit_find_assets_by_account_id(FindAssetsByAccountId),
        visit_find_assets_by_asset_definition_id(FindAssetsByAssetDefinitionId),
        visit_find_assets_by_domain_id(FindAssetsByDomainId),
        visit_find_assets_by_domain_id_and_asset_definition_id(FindAssetsByDomainIdAndAssetDefinitionId),
        visit_find_assets_by_name(FindAssetsByName),
        visit_find_block_header_by_hash(FindBlockHeaderByHash),
        visit_find_domain_by_id(FindDomainById),
        visit_find_domain_key_value_by_id_and_key(FindDomainKeyValueByIdAndKey),
        visit_find_permission_tokens_by_account_id(FindPermissionTokensByAccountId),
        visit_find_role_by_role_id(FindRoleByRoleId),
        visit_find_roles_by_account_id(FindRolesByAccountId),
        visit_find_total_asset_quantity_by_asset_definition_id(FindTotalAssetQuantityByAssetDefinitionId),
        visit_find_transaction_by_hash(FindTransactionByHash),
        visit_find_transactions_by_account_id(FindTransactionsByAccountId),
        visit_find_trigger_by_id(FindTriggerById),
        visit_find_trigger_key_value_by_id_and_key(FindTriggerKeyValueByIdAndKey),
        visit_find_triggers_by_domain_id(FindTriggersByDomainId),
    }
}

pub fn visit_wasm<V: Visit + ?Sized>(
    _visitor: &mut V,
    _authority: &AccountId,
    _wasm: &WasmSmartContract,
) {
}

/// Default validation for [`InstructionExpr`].
///
/// # Warning
///
/// Instruction is executed following successful validation
pub fn visit_instruction<V: Visit + ?Sized>(
    visitor: &mut V,
    authority: &AccountId,
    isi: &InstructionExpr,
) {
    macro_rules! isi_visitors {
        ( $($visitor:ident($isi:ident)),+ $(,)? ) => {
            match isi {
                InstructionExpr::NewParameter(isi) => {
                    let parameter = evaluate_expr!(visitor, authority, <isi as NewParameter>::parameter());
                    visitor.visit_new_parameter(authority, NewParameter{parameter});
                }
                InstructionExpr::SetParameter(isi) => {
                    let parameter = evaluate_expr!(visitor, authority, <isi as NewParameter>::parameter());
                    visitor.visit_set_parameter(authority, SetParameter{parameter});
                }
                InstructionExpr::ExecuteTrigger(isi) => {
                    let trigger_id = evaluate_expr!(visitor, authority, <isi as ExecuteTrigger>::trigger_id());
                    visitor.visit_execute_trigger(authority, ExecuteTrigger{trigger_id});
                }
                InstructionExpr::Log(isi) => {
                    let msg = evaluate_expr!(visitor, authority, <isi as LogExpr>::msg());
                    let level = evaluate_expr!(visitor, authority, <isi as LogExpr>::level());
                    visitor.visit_log(authority, Log { msg, level });
                } $(
                InstructionExpr::$isi(isi) => {
                    visitor.$visitor(authority, isi);
                } )+
            }
        };
    }

    isi_visitors! {
        visit_burn(Burn),
        visit_fail(Fail),
        visit_grant(Grant),
        visit_mint(Mint),
        visit_register(Register),
        visit_remove_key_value(RemoveKeyValue),
        visit_revoke(Revoke),
        visit_set_key_value(SetKeyValue),
        visit_transfer(Transfer),
        visit_unregister(Unregister),
        visit_upgrade(Upgrade),
        visit_sequence(Sequence),
        visit_pair(Pair),
        visit_if(If),
    }
}

pub fn visit_expression<V: Visit + ?Sized, X>(
    visitor: &mut V,
    authority: &AccountId,
    expression: &EvaluatesTo<X>,
) {
    macro_rules! visit_binary_math_expression {
        ($e:ident) => {{
            visitor.visit_expression(authority, $e.left());
            visitor.visit_expression(authority, $e.right())
        }};
    }

    macro_rules! visit_binary_bool_expression {
        ($e:ident) => {{
            visitor.visit_expression(authority, $e.left());
            visitor.visit_expression(authority, $e.right())
        }};
    }

    match expression.expression() {
        Expression::Add(expr) => visit_binary_math_expression!(expr),
        Expression::Subtract(expr) => visit_binary_math_expression!(expr),
        Expression::Multiply(expr) => visit_binary_math_expression!(expr),
        Expression::Divide(expr) => visit_binary_math_expression!(expr),
        Expression::Mod(expr) => visit_binary_math_expression!(expr),
        Expression::RaiseTo(expr) => visit_binary_math_expression!(expr),
        Expression::Greater(expr) => visit_binary_math_expression!(expr),
        Expression::Less(expr) => visit_binary_math_expression!(expr),
        Expression::Equal(expr) => visit_binary_bool_expression!(expr),
        Expression::Not(expr) => visitor.visit_expression(authority, expr.expression()),
        Expression::And(expr) => visit_binary_bool_expression!(expr),
        Expression::Or(expr) => visit_binary_bool_expression!(expr),
        Expression::If(expr) => {
            visitor.visit_expression(authority, expr.condition());
            visitor.visit_expression(authority, expr.then());
            visitor.visit_expression(authority, expr.otherwise())
        }
        Expression::Contains(expr) => {
            visitor.visit_expression(authority, expr.collection());
            visitor.visit_expression(authority, expr.element())
        }
        Expression::ContainsAll(expr) => {
            visitor.visit_expression(authority, expr.collection());
            visitor.visit_expression(authority, expr.elements())
        }
        Expression::ContainsAny(expr) => {
            visitor.visit_expression(authority, expr.collection());
            visitor.visit_expression(authority, expr.elements())
        }
        Expression::Where(expr) => visitor.visit_expression(authority, expr.expression()),
        Expression::Query(query) => visitor.visit_query(authority, query),
        Expression::ContextValue(_) | Expression::Raw(_) => {}
    }
}

pub fn visit_register<V: Visit + ?Sized>(
    visitor: &mut V,
    authority: &AccountId,
    isi: &RegisterExpr,
) {
    macro_rules! match_all {
        ( $( $visitor:ident($object:ident) ),+ $(,)? ) => {
            let object = evaluate_expr!(visitor, authority, <isi as Register>::object());

            match object { $(
                RegistrableBox::$object(object) => visitor.$visitor(authority, Register{object}), )+
            }
        };
    }

    match_all! {
        visit_register_peer(Peer),
        visit_register_domain(Domain),
        visit_register_account(Account),
        visit_register_asset_definition(AssetDefinition),
        visit_register_asset(Asset),
        visit_register_role(Role),
        visit_register_trigger(Trigger),
    }
}

pub fn visit_unregister<V: Visit + ?Sized>(
    visitor: &mut V,
    authority: &AccountId,
    isi: &UnregisterExpr,
) {
    macro_rules! match_all {
        ( $( $visitor:ident($id:ident) ),+ $(,)? ) => {
            let object_id = evaluate_expr!(visitor, authority, <isi as Unregister>::object_id());
            match object_id { $(
                IdBox::$id(object_id) => visitor.$visitor(authority, Unregister{object_id}), )+
                _ => visitor.visit_unsupported(authority, isi),
            }
        };
    }

    match_all! {
        visit_unregister_peer(PeerId),
        visit_unregister_domain(DomainId),
        visit_unregister_account(AccountId),
        visit_unregister_asset_definition(AssetDefinitionId),
        visit_unregister_asset(AssetId),
        visit_unregister_role(RoleId),
        visit_unregister_trigger(TriggerId),
    }
}

pub fn visit_mint<V: Visit + ?Sized>(visitor: &mut V, authority: &AccountId, isi: &MintExpr) {
    let destination_id = evaluate_expr!(visitor, authority, <isi as Mint>::destination_id());
    let object = evaluate_expr!(visitor, authority, <isi as Mint>::object());

    match (destination_id, object) {
        (IdBox::AssetId(destination_id), Value::Numeric(object)) => visitor.visit_mint_asset(
            authority,
            Mint {
                object,
                destination_id,
            },
        ),
        (IdBox::AccountId(destination_id), Value::PublicKey(object)) => visitor
            .visit_mint_account_public_key(
                authority,
                Mint {
                    object,
                    destination_id,
                },
            ),
        (IdBox::AccountId(destination_id), Value::SignatureCheckCondition(object)) => visitor
            .visit_mint_account_signature_check_condition(
                authority,
                Mint {
                    object,
                    destination_id,
                },
            ),
        (IdBox::TriggerId(destination_id), Value::Numeric(NumericValue::U32(object))) => visitor
            .visit_mint_trigger_repetitions(
                authority,
                Mint {
                    object,
                    destination_id,
                },
            ),
        _ => visitor.visit_unsupported(authority, isi),
    }
}

pub fn visit_burn<V: Visit + ?Sized>(visitor: &mut V, authority: &AccountId, isi: &BurnExpr) {
    let destination_id = evaluate_expr!(visitor, authority, <isi as Burn>::destination_id());
    let object = evaluate_expr!(visitor, authority, <isi as Burn>::object());

    match (destination_id, object) {
        (IdBox::AssetId(destination_id), Value::Numeric(object)) => visitor.visit_burn_asset(
            authority,
            Burn {
                object,
                destination_id,
            },
        ),
        (IdBox::AccountId(destination_id), Value::PublicKey(object)) => visitor
            .visit_burn_account_public_key(
                authority,
                Burn {
                    object,
                    destination_id,
                },
            ),
        (IdBox::TriggerId(destination_id), Value::Numeric(NumericValue::U32(object))) => visitor
            .visit_burn_trigger_repetitions(
                authority,
                Burn {
                    object,
                    destination_id,
                },
            ),
        _ => visitor.visit_unsupported(authority, isi),
    }
}

pub fn visit_transfer<V: Visit + ?Sized>(
    visitor: &mut V,
    authority: &AccountId,
    isi: &TransferExpr,
) {
    let object = evaluate_expr!(visitor, authority, <isi as Transfer>::object());
    let source_id = evaluate_expr!(visitor, authority, <isi as Transfer>::source_id());
    let destination_id = evaluate_expr!(visitor, authority, <isi as Transfer>::destination_id());

    match (source_id, object, destination_id) {
        (IdBox::AssetId(source_id), Value::Numeric(object), IdBox::AccountId(destination_id)) => {
            visitor.visit_transfer_asset(
                authority,
                Transfer {
                    source_id,
                    object,
                    destination_id,
                },
            )
        }
        (
            IdBox::AccountId(source_id),
            Value::Id(IdBox::AssetDefinitionId(object)),
            IdBox::AccountId(destination_id),
        ) => visitor.visit_transfer_asset_definition(
            authority,
            Transfer {
                source_id,
                object,
                destination_id,
            },
        ),
        (
            IdBox::AccountId(source_id),
            Value::Id(IdBox::DomainId(object)),
            IdBox::AccountId(destination_id),
        ) => visitor.visit_transfer_domain(
            authority,
            Transfer {
                source_id,
                object,
                destination_id,
            },
        ),
        _ => visitor.visit_unsupported(authority, isi),
    }
}

pub fn visit_set_key_value<V: Visit + ?Sized>(
    visitor: &mut V,
    authority: &AccountId,
    isi: &SetKeyValueExpr,
) {
    let object_id = evaluate_expr!(visitor, authority, <isi as SetKeyValue>::object_id());
    let key = evaluate_expr!(visitor, authority, <isi as SetKeyValue>::key());
    let value = evaluate_expr!(visitor, authority, <isi as SetKeyValue>::value());

    match object_id {
        IdBox::AssetId(object_id) => visitor.visit_set_asset_key_value(
            authority,
            SetKeyValue {
                object_id,
                key,
                value,
            },
        ),
        IdBox::AssetDefinitionId(object_id) => visitor.visit_set_asset_definition_key_value(
            authority,
            SetKeyValue {
                object_id,
                key,
                value,
            },
        ),
        IdBox::AccountId(object_id) => visitor.visit_set_account_key_value(
            authority,
            SetKeyValue {
                object_id,
                key,
                value,
            },
        ),
        IdBox::DomainId(object_id) => visitor.visit_set_domain_key_value(
            authority,
            SetKeyValue {
                object_id,
                key,
                value,
            },
        ),
        _ => visitor.visit_unsupported(authority, isi),
    }
}

pub fn visit_remove_key_value<V: Visit + ?Sized>(
    visitor: &mut V,
    authority: &AccountId,
    isi: &RemoveKeyValueExpr,
) {
    let object_id = evaluate_expr!(visitor, authority, <isi as RemoveKeyValue>::object_id());
    let key = evaluate_expr!(visitor, authority, <isi as RemoveKeyValue>::key());

    match object_id {
        IdBox::AssetId(object_id) => {
            visitor.visit_remove_asset_key_value(authority, RemoveKeyValue { object_id, key })
        }
        IdBox::AssetDefinitionId(object_id) => visitor
            .visit_remove_asset_definition_key_value(authority, RemoveKeyValue { object_id, key }),
        IdBox::AccountId(object_id) => {
            visitor.visit_remove_account_key_value(authority, RemoveKeyValue { object_id, key })
        }
        IdBox::DomainId(object_id) => {
            visitor.visit_remove_domain_key_value(authority, RemoveKeyValue { object_id, key })
        }
        _ => visitor.visit_unsupported(authority, isi),
    }
}

pub fn visit_grant<V: Visit + ?Sized>(visitor: &mut V, authority: &AccountId, isi: &GrantExpr) {
    let destination_id = evaluate_expr!(visitor, authority, <isi as Grant>::destination_id());
    let object = evaluate_expr!(visitor, authority, <isi as Grant>::object());

    match object {
        Value::PermissionToken(object) => visitor.visit_grant_account_permission(
            authority,
            Grant {
                object,
                destination_id,
            },
        ),
        Value::Id(IdBox::RoleId(object)) => visitor.visit_grant_account_role(
            authority,
            Grant {
                object,
                destination_id,
            },
        ),
        _ => visitor.visit_unsupported(authority, isi),
    }
}

pub fn visit_revoke<V: Visit + ?Sized>(visitor: &mut V, authority: &AccountId, isi: &RevokeExpr) {
    let destination_id = evaluate_expr!(visitor, authority, <isi as Revoke>::destination_id());
    let object = evaluate_expr!(visitor, authority, <isi as Revoke>::object());

    match object {
        Value::PermissionToken(object) => visitor.visit_revoke_account_permission(
            authority,
            Revoke {
                object,
                destination_id,
            },
        ),
        Value::Id(IdBox::RoleId(object)) => visitor.visit_revoke_account_role(
            authority,
            Revoke {
                object,
                destination_id,
            },
        ),
        _ => visitor.visit_unsupported(authority, isi),
    }
}

pub fn visit_upgrade<V: Visit + ?Sized>(visitor: &mut V, authority: &AccountId, isi: &UpgradeExpr) {
    let object = evaluate_expr!(visitor, authority, <isi as Upgrade>::object());

    match object {
        UpgradableBox::Executor(object) => {
            visitor.visit_upgrade_executor(authority, Upgrade { object })
        }
    }
}

pub fn visit_if<V: Visit + ?Sized>(visitor: &mut V, authority: &AccountId, isi: &ConditionalExpr) {
    let condition = evaluate_expr!(visitor, authority, <isi as ConditionalExpr>::condition());

    // TODO: Should visit both by default or not? It will affect Executor behavior
    // because only one branch needs to be executed. IMO both should be validated
    if condition {
        visitor.visit_instruction(authority, isi.then());
    } else if let Some(otherwise) = isi.otherwise() {
        visitor.visit_instruction(authority, otherwise);
    }
}

pub fn visit_pair<V: Visit + ?Sized>(visitor: &mut V, authority: &AccountId, isi: &PairExpr) {
    visitor.visit_instruction(authority, isi.left_instruction());
    visitor.visit_instruction(authority, isi.right_instruction());
}

pub fn visit_sequence<V: Visit + ?Sized>(
    visitor: &mut V,
    authority: &AccountId,
    isi: &SequenceExpr,
) {
    for instruction in isi.instructions() {
        visitor.visit_instruction(authority, instruction);
    }
}

macro_rules! leaf_visitors {
    ( $($visitor:ident($operation:ty)),+ $(,)? ) => { $(
        pub fn $visitor<V: Visit + ?Sized>(_visitor: &mut V, _authority: &AccountId, _operation: $operation) {

        } )+
    };
}

leaf_visitors! {
    // Instruction visitors
    visit_register_account(Register<Account>),
    visit_unregister_account(Unregister<Account>),
    visit_mint_account_public_key(Mint<PublicKey, Account>),
    visit_burn_account_public_key(Burn<PublicKey, Account>),
    visit_mint_account_signature_check_condition(Mint<SignatureCheckCondition, Account>),
    visit_set_account_key_value(SetKeyValue<Account>),
    visit_remove_account_key_value(RemoveKeyValue<Account>),
    visit_register_asset(Register<Asset>),
    visit_unregister_asset(Unregister<Asset>),
    visit_mint_asset(Mint<NumericValue, Asset>),
    visit_burn_asset(Burn<NumericValue, Asset>),
    visit_transfer_asset(Transfer<Asset, NumericValue, Account>),
    visit_set_asset_key_value(SetKeyValue<Asset>),
    visit_remove_asset_key_value(RemoveKeyValue<Asset>),
    visit_register_asset_definition(Register<AssetDefinition>),
    visit_unregister_asset_definition(Unregister<AssetDefinition>),
    visit_transfer_asset_definition(Transfer<Account, AssetDefinitionId, Account>),
    visit_set_asset_definition_key_value(SetKeyValue<AssetDefinition>),
    visit_remove_asset_definition_key_value(RemoveKeyValue<AssetDefinition>),
    visit_register_domain(Register<Domain>),
    visit_unregister_domain(Unregister<Domain>),
    visit_transfer_domain(Transfer<Account, DomainId, Account>),
    visit_set_domain_key_value(SetKeyValue<Domain>),
    visit_remove_domain_key_value(RemoveKeyValue<Domain>),
    visit_register_peer(Register<Peer>),
    visit_unregister_peer(Unregister<Peer>),
    visit_grant_account_permission(Grant<PermissionToken>),
    visit_revoke_account_permission(Revoke<PermissionToken>),
    visit_register_role(Register<Role>),
    visit_unregister_role(Unregister<Role>),
    visit_grant_account_role(Grant<RoleId>),
    visit_revoke_account_role(Revoke<RoleId>),
    visit_register_trigger(Register<Trigger<TriggeringFilterBox, Executable>>),
    visit_unregister_trigger(Unregister<Trigger<TriggeringFilterBox, Executable>>),
    visit_mint_trigger_repetitions(Mint<u32, Trigger<TriggeringFilterBox, Executable>>),
    visit_burn_trigger_repetitions(Burn<u32, Trigger<TriggeringFilterBox, Executable>>),
    visit_upgrade_executor(Upgrade<Executor>),
    visit_new_parameter(NewParameter),
    visit_set_parameter(SetParameter),
    visit_execute_trigger(ExecuteTrigger),
    visit_fail(&Fail),
    visit_log(Log),

    // Query visitors
    visit_find_account_by_id(&FindAccountById),
    visit_find_account_key_value_by_id_and_key(&FindAccountKeyValueByIdAndKey),
    visit_find_accounts_by_domain_id(&FindAccountsByDomainId),
    visit_find_accounts_by_name(&FindAccountsByName),
    visit_find_accounts_with_asset(&FindAccountsWithAsset),
    visit_find_all_accounts(&FindAllAccounts),
    visit_find_all_active_trigger_ids(&FindAllActiveTriggerIds),
    visit_find_all_assets(&FindAllAssets),
    visit_find_all_assets_definitions(&FindAllAssetsDefinitions),
    visit_find_all_block_headers(&FindAllBlockHeaders),
    visit_find_all_blocks(&FindAllBlocks),
    visit_find_all_domains(&FindAllDomains),
    visit_find_all_parammeters(&FindAllParameters),
    visit_find_all_peers(&FindAllPeers),
    visit_find_permission_token_schema(&FindPermissionTokenSchema),
    visit_find_all_role_ids(&FindAllRoleIds),
    visit_find_all_roles(&FindAllRoles),
    visit_find_all_transactions(&FindAllTransactions),
    visit_find_asset_by_id(&FindAssetById),
    visit_find_asset_definition_by_id(&FindAssetDefinitionById),
    visit_find_asset_definition_key_value_by_id_and_key(&FindAssetDefinitionKeyValueByIdAndKey),
    visit_find_asset_key_value_by_id_and_key(&FindAssetKeyValueByIdAndKey),
    visit_find_asset_quantity_by_id(&FindAssetQuantityById),
    visit_find_assets_by_account_id(&FindAssetsByAccountId),
    visit_find_assets_by_asset_definition_id(&FindAssetsByAssetDefinitionId),
    visit_find_assets_by_domain_id(&FindAssetsByDomainId),
    visit_find_assets_by_domain_id_and_asset_definition_id(&FindAssetsByDomainIdAndAssetDefinitionId),
    visit_find_assets_by_name(&FindAssetsByName),
    visit_find_block_header_by_hash(&FindBlockHeaderByHash),
    visit_find_domain_by_id(&FindDomainById),
    visit_find_domain_key_value_by_id_and_key(&FindDomainKeyValueByIdAndKey),
    visit_find_permission_tokens_by_account_id(&FindPermissionTokensByAccountId),
    visit_find_role_by_role_id(&FindRoleByRoleId),
    visit_find_roles_by_account_id(&FindRolesByAccountId),
    visit_find_total_asset_quantity_by_asset_definition_id(&FindTotalAssetQuantityByAssetDefinitionId),
    visit_find_transaction_by_hash(&FindTransactionByHash),
    visit_find_transactions_by_account_id(&FindTransactionsByAccountId),
    visit_find_trigger_by_id(&FindTriggerById),
    visit_find_trigger_key_value_by_id_and_key(&FindTriggerKeyValueByIdAndKey),
    visit_find_triggers_by_domain_id(&FindTriggersByDomainId),
}
