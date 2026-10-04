//! Independent owned-frame oracle and atomic ordered-journal controls.

use super::*;

fn frames() -> FastpqExecutionEffectsV1 {
    let mut tape = super::super::tests::effects();
    let FastpqExecutionEffectKindV1::Mint(supply) = tape.effects[0].kind.clone() else {
        unreachable!()
    };
    let mut destination = supply.balance.clone();
    destination.scope =
        AssetBalanceScope::Dataspace(iroha_model_base::topology::DataSpaceId::new(7));
    let transfer = FastpqExecutionTransferV1 {
        source: supply.balance.clone(),
        destination,
        amount: 1u32.into(),
        source_before: 14u32.into(),
        source_after: 13u32.into(),
        destination_before: 0u32.into(),
        destination_after: 1u32.into(),
    };
    let kinds = [
        FastpqExecutionEffectKindV1::Transfer(transfer),
        FastpqExecutionEffectKindV1::Mint(supply.clone()),
        FastpqExecutionEffectKindV1::Burn(supply.clone()),
        FastpqExecutionEffectKindV1::Retire(supply.balance.asset),
    ];
    tape.effects = kinds
        .into_iter()
        .enumerate()
        .map(|(ordinal, kind)| FastpqExecutionEffectV1 {
            ordinal: u32::try_from(ordinal).unwrap(),
            authority_digest: Hash::new(b"original authority"),
            authorization_context: Hash::new(b"original context"),
            kind,
        })
        .collect();
    tape
}

pub(in crate::fastpq::execution_effect) fn owned_oracle(tape: &FastpqExecutionEffectsV1) -> Hash {
    let mut chain = Hash::new(b"fastpq:execution-effects:v1:empty|");
    for (index, effect) in tape.effects.iter().enumerate() {
        let ordinal = u32::try_from(index).unwrap();
        assert_eq!(effect.ordinal, ordinal);
        let frame = norito::encode_canonical(effect).unwrap();
        let item = Hash::new_from_chunks(&[b"fastpq:execution-effects:v1:effect|", &frame]);
        chain = Hash::new_from_chunks(&[
            b"fastpq:execution-effects:v1:step|",
            chain.as_ref(),
            &ordinal.to_le_bytes(),
            item.as_ref(),
        ]);
    }
    let context = Hash::new_from_chunks(&[
        b"fastpq:execution-effects:v1:context|",
        &norito::encode_canonical(&tape.context).unwrap(),
    ]);
    let count = u32::try_from(tape.effects.len()).unwrap();
    Hash::new_from_chunks(&[
        b"fastpq:execution-effects:v1:source|",
        context.as_ref(),
        &count.to_le_bytes(),
        chain.as_ref(),
    ])
}

#[test]
fn borrowed_all_kinds_match_sole_owned_canonical_frame_in_every_layout() {
    let tape = frames();
    for flags in [0, norito::core::default_encode_flags()] {
        let _ambient = norito::core::DecodeFlagsGuard::enter(flags);
        for effect in &tape.effects {
            let borrowed = Effect::from(FastpqExecutionEffectRefV1::from(effect));
            assert_eq!(
                norito::encode_canonical(&borrowed).unwrap(),
                norito::encode_canonical(effect).unwrap()
            );
            assert_eq!(
                norito::canonical_frame_len(&borrowed).unwrap(),
                norito::canonical_frame_len(effect).unwrap()
            );
        }
    }
}

#[test]
fn ordered_journal_matches_independent_owned_frames_at_every_prefix() {
    let mut tape = frames();
    let original = tape.effects.clone();
    let mut journal = FastpqExecutionEffectCommitmentV1::new(tape.context);
    for count in 0..=original.len() {
        tape.effects = original[..count].to_vec();
        assert_eq!(journal.digest().unwrap(), owned_oracle(&tape));
        assert_eq!(
            execution_effects_digest_v1(&tape).unwrap(),
            owned_oracle(&tape)
        );
        assert_eq!(journal.context(), &tape.context);
        assert_eq!(usize::try_from(journal.count()).unwrap(), count);
        assert_ne!(
            journal.digest().unwrap(),
            Hash::new_from_chunks(&[
                b"fastpq:execution-effects:v1:source|",
                &norito::encode_canonical(&tape).unwrap(),
            ]),
            "retired whole-tape digest must not survive as an accepted alias"
        );
        if let Some(effect) = original.get(count) {
            journal.append(effect.into()).unwrap();
        }
    }
}

#[test]
fn ordinal_and_count_refusals_leave_original_journal_unchanged() {
    let tape = frames();
    let mut journal = FastpqExecutionEffectCommitmentV1::new(tape.context);
    let before = journal;
    assert!(matches!(
        journal.append((&tape.effects[1]).into()),
        Err(norito::Error::NonCanonicalEncoding)
    ));
    assert_eq!(journal, before);
    journal.append((&tape.effects[0]).into()).unwrap();
    let before = journal;
    assert!(journal.append((&tape.effects[0]).into()).is_err());
    assert_eq!(journal, before);
    journal.count = u32::MAX;
    let before = journal;
    let mut last = FastpqExecutionEffectRefV1::from(&tape.effects[0]);
    last.ordinal = u32::MAX;
    assert!(matches!(
        journal.append(last),
        Err(norito::Error::LengthMismatch)
    ));
    assert_eq!(journal, before);
}

#[test]
fn ordered_commitment_binds_every_source_and_effect_field() {
    let tape = frames();
    let original = execution_effects_digest_v1(&tape).unwrap();
    for change in 0..15 {
        let mut changed = tape.clone();
        match change {
            0 => changed.context.source.height += 1,
            1 => {
                changed.context.source.network_id = crate::NetworkId::from_genesis_hash(
                    iroha_crypto::HashOf::from_untyped_unchecked(Hash::new(b"other network")),
                )
            }
            2 => changed.context.entry.entry_hash = Hash::new(b"other entry"),
            3 => {
                changed.context.entry.dataspace_id = iroha_model_base::topology::DataSpaceId::new(9)
            }
            4 => changed.effects[0].authority_digest = Hash::new(b"other authority"),
            5 => changed.effects[0].authorization_context = Hash::new(b"other authorization"),
            6 => {
                changed.effects.swap(0, 1);
                for (i, e) in changed.effects.iter_mut().enumerate() {
                    e.ordinal = u32::try_from(i).unwrap();
                }
            }
            7 => {
                changed.effects.pop();
            }
            _ => {
                let FastpqExecutionEffectKindV1::Transfer(v) = &mut changed.effects[0].kind else {
                    unreachable!()
                };
                match change {
                    8 => v.amount = 2u32.into(),
                    9 => v.source_before = 15u32.into(),
                    10 => v.source_after = 12u32.into(),
                    11 => v.destination_before = 2u32.into(),
                    12 => v.destination_after = 3u32.into(),
                    13 => {
                        v.source.asset.incarnation = AxtAssetIncarnationV1::try_from_bytes(
                            Hash::new(b"other lifecycle").into(),
                        )
                        .unwrap()
                    }
                    _ => {
                        v.source.scope = AssetBalanceScope::Dataspace(
                            iroha_model_base::topology::DataSpaceId::new(11),
                        )
                    }
                }
            }
        }
        assert_ne!(
            execution_effects_digest_v1(&changed).unwrap(),
            original,
            "mutation {change}"
        );
    }
}

#[test]
fn borrowed_effect_retains_the_original_static_nominal_and_frame_identity() {
    use norito::NoritoSchema;
    let nominal = FastpqExecutionEffectV1::static_nominal_name().expect("owned literal identity");
    let frame = FastpqExecutionEffectV1::static_frame_name().expect("owned literal frame identity");
    assert_eq!(Effect::static_nominal_name(), Some(nominal));
    assert_eq!(Effect::static_frame_name(), Some(frame));
    assert_eq!(Effect::nominal_name(), nominal);
    assert_eq!(Effect::frame_name(), frame);
}

#[test]
fn borrowed_supply_payload_and_both_tags_preserve_the_owned_bytes() {
    let tape = frames();
    let FastpqExecutionEffectKindV1::Mint(mut supply) = tape.effects[1].kind.clone() else {
        unreachable!()
    };
    // Distinct original values expose an accidental before/after field swap.
    supply.balance_before = 29u32.into();
    supply.balance_after = 31u32.into();
    supply.supply_before = 103u32.into();
    supply.supply_after = 107u32.into();
    for flags in [0, norito::core::default_encode_flags()] {
        let _ambient = norito::core::DecodeFlagsGuard::enter(flags);
        let borrowed = SupplyChange::from(FastpqExecutionSupplyChangeRefV1 {
            balance: (&supply.balance).into(),
            amount: &supply.amount,
            balance_before: &supply.balance_before,
            balance_after: &supply.balance_after,
            supply_before: &supply.supply_before,
            supply_after: &supply.supply_after,
        });
        let payload = norito::codec::Encode::encode(&borrowed);
        assert_eq!(payload, norito::codec::Encode::encode(&supply));
        let mut swapped = supply.clone();
        std::mem::swap(&mut swapped.supply_before, &mut swapped.supply_after);
        assert_ne!(payload, norito::codec::Encode::encode(&swapped));
        for kind in [
            FastpqExecutionEffectKindV1::Mint(supply.clone()),
            FastpqExecutionEffectKindV1::Burn(supply.clone()),
        ] {
            let effect = FastpqExecutionEffectV1 {
                ordinal: 0,
                authority_digest: tape.effects[1].authority_digest,
                authorization_context: tape.effects[1].authorization_context,
                kind,
            };
            let projected = Effect::from(FastpqExecutionEffectRefV1::from(&effect));
            assert_eq!(
                norito::encode_canonical(&projected).unwrap(),
                norito::encode_canonical(&effect).unwrap()
            );
        }
    }
}
