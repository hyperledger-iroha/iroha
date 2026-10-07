//! Durable publisher faults with explicit mock proof authority and actual P-256 signatures.
use super::*;
use crate::kagemusha_wallet_state_v1::tests::{bootstrap, field, fixture, frozen, signer, wallet};
use p256::ecdsa::{Signature, signature::Signer as _};

pub(super) fn output(plan: &Plan) -> Vec<u8> {
    let body = plan.body().unwrap();
    let raw: Signature = signer(&plan.credential).sign(&body.signing_message());
    let control = KagemushaWalletLedgerControlV1::sign(
        body,
        &plan.credential.body.payment_key,
        KagemushaWalletSignerOutputV1::Raw(raw.to_bytes().into()),
    )
    .unwrap();
    KagemushaWalletActivationV1 {
        version: 1,
        control,
        credential: plan.credential,
        bootstrap: plan.bootstrap.clone(),
        asset: plan.asset.clone(),
        certificates: plan.certificates.clone(),
    }
    .to_canonical_bytes()
    .unwrap()
}

#[test]
fn activation_publisher_retains_original_through_uncertainty_restart_and_new_head() {
    let asset = fixture("KagemushaWalletAssetScopeV1");
    for after in [false, true] {
        let mut w = wallet();
        w.commit(bootstrap()).unwrap();
        let plan = w.activation_plan(&asset).unwrap();
        let exact = output(&plan);
        w.custody.fail_publication = Some(after);
        assert!(w.finish_activation(&plan, &exact).is_err());
        let mut w =
            Coordinator::new(w.custody, w.archive, w.proofs, w.scheme_id, w.wallet_id).unwrap();
        let retained = w.activation_plan(&asset).unwrap();
        assert_eq!(retained.nonce, plan.nonce);
        if !after {
            assert!(retained.output.is_none());
            w.finish_activation(&retained, &exact).unwrap();
        }
        let done = w.activation_plan(&asset).unwrap();
        let address = done.output.unwrap();
        assert_eq!(
            w.archive
                .read_object(&address, KAGEMUSHA_WALLET_ACTIVATION_MAX_BYTES_V1)
                .unwrap(),
            exact
        );
        assert!(w.finish_activation(&plan, &exact).is_err());
        // A later source cannot replace the selected sequence-zero transport.
        let (_, manifest) = w.manifest().unwrap();
        let pred = w.indexed_step(&manifest, 0).unwrap();
        let next = frozen(
            Some(&pred.frozen),
            KagemushaWalletEffectV1::Load {
                load_ordinal: 0,
                receipt_digest: field(31),
                amount: 1,
                online_charge: 0,
            },
        );
        w.commit(next).unwrap();
        let done = w.activation_plan(&asset).unwrap();
        assert_eq!(done.output, Some(address));
        assert_eq!(done.nonce, plan.nonce);
        w.archive.remove(ArchiveKey::Object(address)).unwrap();
        assert!(matches!(
            w.archive
                .read_object(&address, KAGEMUSHA_WALLET_ACTIVATION_MAX_BYTES_V1),
            Err(Error::WitnessLost(_))
        ));
        let lost = w.activation_plan(&asset).unwrap();
        assert_eq!(lost.output, Some(address));
        assert!(matches!(
            w.retained_activation(&lost),
            Err(Error::WitnessLost(_))
        ));
    }
}

#[test]
fn activation_binds_bootstrap_originals_nonce_asset_and_signed_output() {
    let mut w = wallet();
    w.commit(bootstrap()).unwrap();
    let asset = fixture("KagemushaWalletAssetScopeV1");
    let plan = w.activation_plan(&asset).unwrap();
    let exact = output(&plan);
    plan.output(&exact).unwrap();
    let mut changed = plan.clone();
    changed.nonce = field(9);
    assert!(changed.output(&exact).is_err());
    changed = plan.clone();
    changed.bootstrap.statement.sequence = 1;
    assert!(changed.require(&w.scheme_id, &w.wallet_id, &asset).is_err());
    changed = plan.clone();
    changed.capsule = field(11);
    assert!(changed.require(&w.scheme_id, &w.wallet_id, &asset).is_err());
    assert!(plan.require(&field(13), &w.wallet_id, &asset).is_err());
    assert!(plan.require(&w.scheme_id, &field(13), &asset).is_err());
    let mut bytes = exact.clone();
    bytes.push(0);
    assert!(w.finish_activation(&plan, &bytes).is_err());
    assert!(w.activation_plan(&asset).unwrap().output.is_none());
}

#[test]
fn collection_requires_the_complete_selected_activation_copy() {
    let mut w = wallet();
    w.commit(bootstrap()).unwrap();
    w.scheduler().set_activity(true, false);
    w.fold_once().unwrap();
    w.fold_once().unwrap();
    let (_, manifest) = w.manifest().unwrap();
    let pred = w.indexed_step(&manifest, 0).unwrap();
    w.commit(frozen(
        Some(&pred.frozen),
        KagemushaWalletEffectV1::Load {
            load_ordinal: 0,
            receipt_digest: field(31),
            amount: 1,
            online_charge: 0,
        },
    ))
    .unwrap();
    w.fold_once().unwrap();
    w.fold_once().unwrap();
    assert!(matches!(
        w.collect_step(0, None),
        Err(Error::Invalid(
            "Bootstrap activation originals are not retained"
        ))
    ));
    let asset = fixture("KagemushaWalletAssetScopeV1");
    let plan = w.activation_plan(&asset).unwrap();
    let bytes = output(&plan);
    w.finish_activation(&plan, &bytes).unwrap();
    for _ in 0..16 {
        if matches!(
            w.collect_step(0, None).unwrap(),
            CollectionStatus::Collected(0)
        ) {
            break;
        }
    }
    let plan = w.activation_plan(&asset).unwrap();
    assert_eq!(w.retained_activation(&plan).unwrap(), Some(bytes));
}
