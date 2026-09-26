//! Canonical candidate effect frames and domain-separated exact quantity identities.

use super::*;
use crate::{
    NetworkId,
    fastpq::{FastpqSourceExecutionKindV1, FastpqSourceRouteV1, transfer_balance_key},
};
use iroha_crypto::{Algorithm, HashOf, KeyPair};
use iroha_model_base::{domain::DomainId, topology::DataSpaceId};

fn balance() -> FastpqExecutionBalanceV1 {
    let key = KeyPair::from_seed(vec![1; 32], Algorithm::Ed25519);
    FastpqExecutionBalanceV1 {
        asset: FastpqExecutionAssetV1 {
            definition: AssetDefinitionId::derive_from_components(
                DomainId::try_new("wonderland", "universal").unwrap(),
                "rose".parse().unwrap(),
            ),
            incarnation: AxtAssetIncarnationV1::try_from_bytes(
                Hash::new(b"registration token").into(),
            )
            .unwrap(),
        },
        account: AccountId::new(key.public_key().clone()),
        scope: AssetBalanceScope::Global,
    }
}
fn effects() -> FastpqExecutionEffectsV1 {
    FastpqExecutionEffectsV1 {
        context: FastpqExecutionEffectContextV1 {
            source: FastpqSourceStatementContextV1 {
                network_id: NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(
                    Hash::new(b"network"),
                )),
                height: 9,
            },
            entry: FastpqSourceExecutionEntryV1 {
                entry_hash: Hash::new(b"original call"),
                execution_kind: FastpqSourceExecutionKindV1::ExecutionCall,
                route: FastpqSourceRouteV1::Unrouted,
                dataspace_id: DataSpaceId::UNIVERSAL,
            },
        },
        effects: vec![FastpqExecutionEffectV1 {
            ordinal: 0,
            authority_digest: Hash::new(b"authority"),
            authorization_context: Hash::new(b"authorization"),
            kind: FastpqExecutionEffectKindV1::Mint(FastpqExecutionSupplyChangeV1 {
                balance: balance(),
                amount: 5u32.into(),
                balance_before: 9u32.into(),
                balance_after: 14u32.into(),
                supply_before: 10u32.into(),
                supply_after: 15u32.into(),
            }),
        }],
    }
}
#[test]
fn quantity_key_tag_scope_incarnation_and_predecessor_are_disjoint() {
    let balance = balance();
    let key =
        execution_quantity_key_v1(&FastpqExecutionQuantityKeyV1::Balance(balance.clone())).unwrap();
    assert!(key.starts_with(b"iroha:fastpq:execution-quantity-key:v1\0"));
    assert_ne!(
        key,
        execution_quantity_key_v1(&FastpqExecutionQuantityKeyV1::Supply(balance.asset.clone()))
            .unwrap()
    );
    assert_ne!(
        key,
        transfer_balance_key(&balance.asset.definition, &balance.account).unwrap()
    );
    let mut scoped = balance.clone();
    scoped.scope = AssetBalanceScope::Dataspace(DataSpaceId::new(7));
    assert_ne!(
        key,
        execution_quantity_key_v1(&FastpqExecutionQuantityKeyV1::Balance(scoped)).unwrap()
    );
    let mut new = balance;
    new.asset.incarnation =
        AxtAssetIncarnationV1::try_from_bytes(Hash::new(b"re-registration token").into()).unwrap();
    assert_ne!(
        key,
        execution_quantity_key_v1(&FastpqExecutionQuantityKeyV1::Balance(new)).unwrap()
    );
}
#[test]
fn effect_statement_canonical_roundtrip_and_complete_commitment() {
    let root = Hash::new(b"root").into();
    let statement = FastpqExecutionEffectStatementV1 {
        public_inputs: FastpqPublicInputs {
            dsid: [0; 16],
            slot: 9,
            old_root: root,
            new_root: root,
            perm_root: root,
            tx_set_hash: root,
        },
        ordering_hash: root,
        transitions: Vec::new(),
        effects: effects(),
    };
    let bytes = norito::encode_canonical(&statement).unwrap();
    assert_eq!(
        norito::decode_canonical::<FastpqExecutionEffectStatementV1>(&bytes).unwrap(),
        statement
    );
    let digest = execution_effect_statement_digest_v1(&statement).unwrap();
    let mut changed = statement.clone();
    changed.effects.effects[0].authorization_context = Hash::new(b"another authority context");
    assert_ne!(
        digest,
        execution_effect_statement_digest_v1(&changed).unwrap()
    );
    changed = statement.clone();
    changed.public_inputs.slot += 1;
    assert_ne!(
        digest,
        execution_effect_statement_digest_v1(&changed).unwrap()
    );
    let mut trailing = bytes;
    trailing.push(0);
    assert!(norito::decode_canonical::<FastpqExecutionEffectStatementV1>(&trailing).is_err());
}
#[test]
fn original_order_operation_identity_and_full_source_are_committed() {
    let original = effects();
    let digest = execution_effects_digest_v1(&original).unwrap();
    for change in 0..6 {
        let mut changed = original.clone();
        match change {
            0 => changed.effects[0].ordinal = 1,
            1 => changed.context.source.height += 1,
            2 => changed.context.entry.entry_hash = Hash::new(b"another call"),
            3 => changed.effects[0].authority_digest = Hash::new(b"another authority"),
            4 => changed.effects.push(changed.effects[0].clone()),
            _ => {
                if let FastpqExecutionEffectKindV1::Mint(m) = &changed.effects[0].kind {
                    changed.effects[0].kind = FastpqExecutionEffectKindV1::Burn(m.clone());
                }
            }
        }
        assert_ne!(digest, execution_effects_digest_v1(&changed).unwrap());
    }
}
