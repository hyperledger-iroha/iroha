// Fixed Ironwood profile regressions, included inside the native engine test module.
pub fn build_ironwood_fixture(
    binding: &PrivacyNativeConsensusBindingV1,
) -> (OrchardBundlePublicV1, Vec<u8>) {
    let recipient = FullViewingKey::from(&spending_key(0x62)).address_at(0_u32, Scope::External);
    let prepared = prepare_ironwood_bundle_v1_with_rng(
        orchard_empty_root_v1(),
        vec![],
        vec![IronwoodOutputProverInputV1::new(
            None,
            recipient,
            17,
            &[0x62; 512],
        )],
        1,
        &mut ProverEntropyRngV1(ProverEntropyModeV1::Healthy),
    )
    .expect("zero-spend recipient deposit");
    let proved = authorize_ironwood_bundle_v1(prepared, binding, &consensus_limits()).unwrap();
    (proved.public, proved.authorization)
}
fn ironwood_fixture() -> &'static (OrchardBundlePublicV1, Vec<u8>) {
    static FIXTURE: OnceLock<(OrchardBundlePublicV1, Vec<u8>)> = OnceLock::new();
    FIXTURE.get_or_init(|| build_ironwood_fixture(&consensus_binding(0x44)))
}
fn ironwood_spend() -> (OrchardSpendProverInputV1, [u8; 32]) {
    let key = spending_key(0x61);
    let address = FullViewingKey::from(&key).address_at(0_u32, Scope::External);
    let rho = Option::<Rho>::from(Rho::from_bytes(&[3; 32])).unwrap();
    let seed = Option::<RandomSeed>::from(RandomSeed::from_bytes([4; 32], &rho)).unwrap();
    let note = Option::<Note>::from(Note::from_parts(
        address,
        NoteValue::from_raw(46),
        rho,
        seed,
        NoteVersion::V3,
    ))
    .unwrap();
    let path = MerklePath::from_parts(
        9,
        [[0; 32]; 32]
            .map(|b| Option::<MerkleHashOrchard>::from(MerkleHashOrchard::from_bytes(&b)).unwrap()),
    );
    let anchor = path
        .root(ExtractedNoteCommitment::from(note.commitment()))
        .to_bytes();
    (OrchardSpendProverInputV1::new(key, note, path), anchor)
}
#[test]
fn ironwood_spend_pays_foreign_recipient_and_returns_payer_change() {
    let payer = FullViewingKey::from(&spending_key(0x61));
    let merchant = FullViewingKey::from(&spending_key(0x62));
    let recipient = merchant.address_at(0_u32, Scope::External);
    let change = payer.address_at(0_u32, Scope::External);
    assert_ne!(recipient, change);
    let (spend, anchor) = ironwood_spend();
    let prepared = prepare_ironwood_bundle_v1_with_rng(
        anchor,
        vec![spend],
        vec![
            IronwoodOutputProverInputV1::new(None, recipient, 17, &[0x62; 512]),
            IronwoodOutputProverInputV1::new(None, change, 29, &[0x61; 512]),
        ],
        1,
        &mut ProverEntropyRngV1(ProverEntropyModeV1::Healthy),
    )
    .unwrap();
    assert_eq!(
        prepared.public_draft().actions.len(),
        2,
        "max(spends, outputs), not their sum"
    );
    assert_eq!(prepared.public_draft().value_balance, 0);
    for (viewer, expected_address, value, memo) in
        [(merchant, recipient, 17, 0x62), (payer, change, 29, 0x61)]
    {
        let outputs = prepared
            .0
            .proven
            .decrypt_outputs_with_keys(&[viewer.to_ivk(Scope::External)]);
        assert_eq!(outputs.len(), 1);
        assert_eq!(outputs[0].2.value().inner(), value);
        assert_eq!(outputs[0].3, expected_address);
        assert_eq!(outputs[0].4, [memo; 512]);
    }
    let proved =
        authorize_ironwood_bundle_v1(prepared, &consensus_binding(0x55), &consensus_limits())
            .unwrap();
    assert_eq!(proved.authorization.len(), 7461);
    verify_ironwood_bundle_v1(&proved.public, &proved.authorization, &consensus_limits()).unwrap();
    let mut changed = proved.public.clone();
    changed.anchor = orchard_empty_root_v1();
    assert!(
        verify_ironwood_bundle_v1(&changed, &proved.authorization, &consensus_limits()).is_err()
    );
}
#[test]
fn ironwood_deposit_and_orchard_authorizations_reject_cross_profile_replay() {
    let (new_public, new_proof) = ironwood_fixture();
    let (old_public, old_proof) = fixture();
    assert_eq!(new_public.value_balance, -17);
    assert_eq!(new_proof.len(), 5125);
    assert_eq!(&new_proof[..4], b"IRW1");
    verify_ironwood_bundle_v1(new_public, new_proof, &consensus_limits()).unwrap();
    for (public, proof, old) in [
        (new_public, new_proof, true),
        (old_public, old_proof, false),
    ] {
        let verify = if old {
            verify_orchard_bundle_v1
        } else {
            verify_ironwood_bundle_v1
        };
        assert_eq!(
            verify(public, proof, &consensus_limits()),
            Err(OrchardNativeErrorV1::AuthorizationWireMagic)
        );
        let mut relabelled = proof.clone();
        relabelled[..4].copy_from_slice(if old { b"ORC1" } else { b"IRW1" });
        assert!(
            verify(public, &relabelled, &consensus_limits()).is_err(),
            "changing only the magic cannot change profile"
        );
    }
    assert_ne!(
        derive_orchard_bundle_sighash_v1(new_public, &consensus_limits()).unwrap(),
        derive_ironwood_bundle_sighash_v1(new_public, &consensus_limits()).unwrap()
    );
    let mut changed = new_public.clone();
    changed.value_balance += 1;
    assert!(verify_ironwood_bundle_v1(&changed, new_proof, &consensus_limits()).is_err());
    for malformed in [
        new_proof[..new_proof.len() - 1].to_vec(),
        [new_proof.as_slice(), &[0]].concat(),
    ] {
        assert!(matches!(
            verify_ironwood_bundle_v1(new_public, &malformed, &consensus_limits()),
            Err(OrchardNativeErrorV1::ProofLength { .. })
        ));
    }
    let mut changed = new_public.clone();
    changed.actions.push(changed.actions[0].clone());
    changed.actions.push(changed.actions[0].clone());
    assert!(matches!(
        verify_ironwood_bundle_v1(&changed, new_proof, &consensus_limits()),
        Err(OrchardNativeErrorV1::ActionCount { actual: 3, max: 2 })
    ));
}
#[test]
fn ironwood_input_bounds_anchor_ownership_and_entropy_fail_closed() {
    let output = || {
        IronwoodOutputProverInputV1::new(
            None,
            FullViewingKey::from(&spending_key(0x62)).address_at(0_u32, Scope::External),
            17,
            &[0; 512],
        )
    };
    let mut rng = ProverEntropyRngV1(ProverEntropyModeV1::Healthy);
    assert!(matches!(
        prepare_ironwood_bundle_v1_with_rng(orchard_empty_root_v1(), vec![], vec![], 1, &mut rng),
        Err(OrchardProverErrorV1::EmptyOperation)
    ));
    assert!(matches!(
        prepare_ironwood_bundle_v1_with_rng(
            orchard_empty_root_v1(),
            vec![],
            vec![output(), output(), output()],
            1,
            &mut rng
        ),
        Err(OrchardProverErrorV1::ActionCount { actual: 3, max: 2 })
    ));
    for padding in [0, 3] {
        assert!(matches!(
            prepare_ironwood_bundle_v1_with_rng(
                orchard_empty_root_v1(),
                vec![],
                vec![output()],
                padding,
                &mut rng
            ),
            Err(OrchardProverErrorV1::MinimumActionCount { .. })
        ));
    }
    assert!(matches!(
        prepare_ironwood_bundle_v1_with_rng([255; 32], vec![], vec![output()], 1, &mut rng),
        Err(OrchardProverErrorV1::AnchorEncoding)
    ));
    let (spend, _) = ironwood_spend();
    assert!(matches!(
        prepare_ironwood_bundle_v1_with_rng(
            orchard_empty_root_v1(),
            vec![spend],
            vec![output()],
            1,
            &mut rng
        ),
        Err(OrchardProverErrorV1::SpendInput { index: 0 })
    ));
    let (mut spend, anchor) = ironwood_spend();
    spend.spending_key = spending_key(0x62);
    assert!(matches!(
        prepare_ironwood_bundle_v1_with_rng(anchor, vec![spend], vec![output()], 1, &mut rng),
        Err(OrchardProverErrorV1::SpendInput { index: 0 })
    ));
    assert!(
        prepare_ironwood_bundle_v1_with_rng(
            orchard_empty_root_v1(),
            vec![],
            vec![output()],
            1,
            &mut ProverEntropyRngV1(ProverEntropyModeV1::Constant)
        )
        .is_err()
    );
}
#[test]
fn ironwood_rejects_mismatched_flags_even_with_valid_profile_signatures() {
    let version = BundleVersion::ironwood_v3();
    assert_eq!(Flags::from_byte(0x87, version), None, "reserved flag bits");
    assert_eq!(Flags::from_byte(7, BundleVersion::orchard_v3()), None);
    let mut rng = StdRng::from_seed([0x65; 32]);
    let builder = Builder::new(
        BundleType::Transactional {
            bundle_required: true,
            pad_to_minimum: Some(1),
        },
        version,
        Flags::CROSS_ADDRESS_DISABLED,
        Anchor::empty_tree(),
    )
    .unwrap();
    let proven = builder
        .build::<i64>(&mut rng)
        .unwrap()
        .unwrap()
        .0
        .create_proof(orchard_v3_proving_key(), &mut rng)
        .unwrap();
    let draft = public_draft_from_bundle_v1(&proven);
    let public = OrchardBundlePublicV1 {
        consensus_binding: consensus_binding(0x44),
        anchor: draft.anchor,
        value_balance: draft.value_balance,
        actions: draft.actions,
    };
    let sighash = derive_ironwood_bundle_sighash_v1(&public, &consensus_limits()).unwrap();
    let authorized = proven.apply_signatures(&mut rng, sighash, &[]).unwrap();
    assert!(matches!(
        encode_authorized_bundle_for_profile_v1(
            &authorized,
            &public.consensus_binding,
            BundleProfileV1::Ironwood
        ),
        Err(OrchardProverErrorV1::OutputShape)
    ));
    // Bypass only the encoder in this adversarial fixture. Signatures are valid for Ironwood,
    // so rejection must come from the Halo2 public flags, not from framing or the sighash.
    let mut wire = b"IRW1\x01".to_vec();
    wire.extend_from_slice(authorized.authorization().proof().as_ref());
    for action in authorized.actions().iter() {
        wire.extend_from_slice(&<[u8; 64]>::from(action.authorization()));
    }
    wire.extend_from_slice(&<[u8; 64]>::from(
        authorized.authorization().binding_signature(),
    ));
    assert_eq!(
        verify_ironwood_bundle_v1(&public, &wire, &consensus_limits()),
        Err(OrchardNativeErrorV1::Halo2Proof)
    );
}
