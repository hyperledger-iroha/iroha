//! AXT transaction, issuer, budget, replay and proof-binding contract regressions.

use super::*;
use crate::domain::DomainId;
use iroha_crypto::{Algorithm, KeyPair};
use iroha_primitives::{bigint::BigInt, numeric::Numeric};

use mv::json::JsonKeyCodec;
use norito::{decode_from_bytes, to_bytes};
fn ordered_set_entry(seed: u8) -> TransactionEntrypoint {
    let signer = KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519);
    let mut builder = crate::transaction::TransactionBuilder::new(
        test_network_id(b"ordered-axt-set"),
        crate::account::AccountId::new(signer.public_key().clone()),
        crate::transaction::FeePaymentIntent::authority(Vec::new(), None),
    );
    builder.set_creation_time(std::time::Duration::from_millis(1_000));
    TransactionEntrypoint::External(builder.try_sign(signer.private_key()).expect("sign entry"))
}

#[test]
fn axt_ordered_transaction_set_matches_exact_wires_and_order() {
    let entries = [ordered_set_entry(61), ordered_set_entry(62)];
    let mut expected = AXT_ORDERED_TRANSACTION_SET_DOMAIN_V1.to_vec();
    expected.extend_from_slice(&2_u64.to_le_bytes());
    for entry in &entries {
        let wire = entry.encode_wire_v1().expect("canonical entry wire");
        expected.extend_from_slice(&(wire.len() as u64).to_le_bytes());
        expected.extend_from_slice(&wire);
    }
    let digest = axt_ordered_transaction_set_digest_v1(&entries).expect("ordered set");
    assert_eq!(digest, Hash::new(&expected));
    assert_ne!(
        digest,
        axt_ordered_transaction_set_digest_v1(entries.iter().rev()).unwrap()
    );
    assert_ne!(
        digest,
        axt_ordered_transaction_set_digest_v1(&entries[..1]).unwrap()
    );
    let _alternate = norito::core::DecodeFlagsGuard::enter(
        norito::core::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN,
    );
    assert_eq!(
        digest,
        axt_ordered_transaction_set_digest_v1(&entries).unwrap()
    );
}

#[test]
fn axt_ordered_transaction_set_retains_authorization_proofs() {
    let original = ordered_set_entry(63);
    let mut changed = original.clone();
    let TransactionEntrypoint::External(transaction) = &mut changed else {
        unreachable!()
    };
    let signer = KeyPair::from_seed(vec![64; 32], Algorithm::Ed25519);
    let signatures = crate::transaction::signed::MultisigSignatures::from_signers(
        transaction.payload(),
        [signer.private_key()],
    )
    .expect("additional authorization proof");
    transaction.set_multisig_signatures(signatures);
    assert_eq!(
        original.execution_call_hash(),
        changed.execution_call_hash()
    );
    assert_ne!(
        axt_ordered_transaction_set_digest_v1([&original]).unwrap(),
        axt_ordered_transaction_set_digest_v1([&changed]).unwrap(),
    );
}

#[test]
fn axt_ordered_transaction_set_enforces_count_and_cumulative_wire_caps() {
    let entries = [ordered_set_entry(65), ordered_set_entry(66)];
    let bytes = entries
        .iter()
        .map(|entry| entry.encode_wire_v1().unwrap().len() as u64)
        .sum();
    assert!(matches!(
        axt_ordered_transaction_set_digest_with_limits(&entries, 1, bytes),
        Err(AxtOrderedTransactionSetErrorV1::Count { maximum: 1 }),
    ));
    assert!(matches!(
        axt_ordered_transaction_set_digest_with_limits(&entries, 2, bytes - 1),
        Err(AxtOrderedTransactionSetErrorV1::WireBytes { maximum }) if maximum == bytes - 1,
    ));
    assert_eq!(
        axt_ordered_transaction_set_digest_with_limits(&entries, 2, bytes).unwrap(),
        axt_ordered_transaction_set_digest_v1(&entries).unwrap(),
    );
    let mut writer = AxtWireBudgetWriter {
        written: u64::MAX,
        maximum: u64::MAX,
        rejected: false,
    };
    assert!(std::io::Write::write(&mut writer, &[1]).is_err());
    assert!(writer.rejected);
}

#[test]
fn axt_ordered_transaction_set_hashing_does_not_impose_proof_witness_count_cap() {
    let entry = ordered_set_entry(67);
    let entries = std::iter::repeat_n(&entry, MAX_AXT_FINALIZED_TRANSACTIONS_V1 + 1);
    assert!(axt_ordered_transaction_set_digest_v1(entries).is_ok());
}

#[test]
fn axt_ordered_transaction_set_rejects_changed_iterator_count_between_passes() {
    struct ChangingCount<'a> {
        entry: &'a TransactionEntrypoint,
        remaining: usize,
        cloned_remaining: usize,
    }
    impl Clone for ChangingCount<'_> {
        fn clone(&self) -> Self {
            Self {
                entry: self.entry,
                remaining: self.cloned_remaining,
                cloned_remaining: self.cloned_remaining,
            }
        }
    }
    impl<'a> Iterator for ChangingCount<'a> {
        type Item = &'a TransactionEntrypoint;
        fn next(&mut self) -> Option<Self::Item> {
            if self.remaining == 0 {
                return None;
            }
            self.remaining -= 1;
            Some(self.entry)
        }
    }
    let entry = ordered_set_entry(68);
    for (remaining, cloned_remaining) in [(1, 2), (2, 1)] {
        assert!(matches!(
            axt_ordered_transaction_set_digest_v1(ChangingCount { entry: &entry, remaining, cloned_remaining }),
            Err(AxtOrderedTransactionSetErrorV1::Encoding(message)) if message.contains("entry count"),
        ));
    }
}

fn sample_descriptor(dsid: DataSpaceId) -> AxtDescriptor {
    AxtDescriptor {
        dsids: vec![dsid],
        touches: vec![AxtTouchSpec {
            dsid,
            read: vec!["orders".into()],
            write: vec!["ledger".into()],
        }],
    }
}
fn test_network_id(seed: &[u8]) -> NetworkId {
    NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
        seed,
    )))
}
fn test_asset_definition_id() -> AssetDefinitionId {
    AssetDefinitionId::derive_from_components(
        DomainId::try_new("axt", "universal").expect("domain"),
        "rose".parse().expect("asset name"),
    )
}
fn test_asset_incarnation(seed: &[u8]) -> AxtAssetIncarnationV1 {
    let network_id = test_network_id(seed);
    let registration_header_hash = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
        [b"axt-test-replay-registration:".as_slice(), seed].concat(),
    ));
    AxtAssetIncarnationV1::derive(
        &network_id,
        &test_asset_definition_id(),
        &registration_header_hash,
        &Hash::new([b"axt-test-replay-execution:".as_slice(), seed].concat()),
        0,
    )
}
fn issuer_context(network_id: NetworkId, asset_dsid: DataSpaceId) -> AxtHandleIssuerContextV1 {
    let asset_definition_id = test_asset_definition_id();
    let registration_header_hash = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
        b"axt-test-asset-registration-header",
    ));
    let execution_identity = Hash::new(b"axt-test-asset-registration-execution");
    AxtHandleIssuerContextV1 {
        network_id,
        asset_dsid,
        asset_definition_incarnation: AxtAssetIncarnationV1::derive(
            &network_id,
            &asset_definition_id,
            &registration_header_hash,
            &execution_identity,
            0,
        ),
        issuer: UniversalAccountId::from_hash(Hash::new(b"axt-test-issuer")),
        issuer_manifest_root: [0x5A; 32],
        code_root: [0xC0; 32],
        abi_version: 1,
        abi_hash: [0xAB; 32],
    }
}
fn sample_asset_handle_draft() -> AssetHandleDraft {
    AssetHandleDraft {
        asset_definition_id: test_asset_definition_id(),
        scope: vec!["transfer".into()],
        subject: HandleSubject {
            account: "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV".into(),
            origin_dsid: Some(DataSpaceId::new(7)),
        },
        budget: HandleBudget {
            remaining: Quantity::from(50_u64),
            per_use: Some(Quantity::from(10_u64)),
        },
        handle_era: 9,
        sub_nonce: 3,
        group_binding: GroupBinding {
            composability_group_id: b"settlement".to_vec(),
            epoch_id: 4,
        },
        target_lane: LaneId::new(2),
        axt_binding: AxtBinding::new([0xA5; 32]),
        manifest_view_root: [0x5A; 32],
        expiry_slot: 100,
        max_clock_skew_ms: Some(25),
    }
}
fn sample_asset_handle() -> AssetHandle {
    let issuer = KeyPair::from_seed(vec![0x33; 32], Algorithm::Ed25519);
    sample_asset_handle_draft()
        .sign_by_issuer_v1(
            issuer_context(test_network_id(b"sequence-network"), DataSpaceId::new(7)),
            issuer.private_key(),
        )
        .expect("sign sample handle")
}
fn sample_finalized_spend_anchor(
    network_id: NetworkId,
    dataspace_id: DataSpaceId,
    lane_id: LaneId,
) -> AxtFinalizedSpendAnchorV1 {
    AxtFinalizedSpendAnchorV1 {
        network_id,
        genesis_hash: *network_id.as_bytes(),
        dataspace_id,
        lane_id,
        lane_incarnation: Hash::new(b"axt-finalized-anchor-lane-incarnation"),
        finalized_height: 42,
        block_header_hash: HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
            b"axt-finalized-anchor-block-header",
        )),
        quorum_certificate_digest: Hash::new(b"axt-finalized-anchor-commit-qc"),
        committee_digest: Hash::new(b"axt-finalized-anchor-committee"),
        pre_state_root: Hash::new(b"axt-finalized-anchor-pre-state"),
        post_state_root: Hash::new(b"axt-finalized-anchor-post-state"),
        transaction_set_digest: Hash::new(b"axt-finalized-anchor-transaction-set"),
        da_manifest_digest: Hash::new(b"axt-finalized-anchor-da-manifest"),
    }
}
fn sample_anchored_spend_draft(
    handle: AssetHandle,
    anchor: AxtFinalizedSpendAnchorV1,
) -> AxtAnchoredSpendDraftV1 {
    let binding = sample_fastpq_binding(anchor.dataspace_id);
    let envelope = AxtProofEnvelope {
        dsid: anchor.dataspace_id,
        manifest_root: handle.manifest_view_root,
        da_commitment: Some(anchor.da_manifest_digest.into()),
        proof: vec![0xA5, 0x5A],
        fastpq_binding: Some(binding),
        committed_amount: Some(5),
        amount_commitment: None,
    };
    AxtAnchoredSpendDraftV1 {
        intent: RemoteSpendIntent {
            asset_dsid: anchor.dataspace_id,
            op: SpendOp {
                asset_definition_id: handle.asset_definition_id.clone(),
                kind: "transfer".to_owned(),
                from: "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV".to_owned(),
                to: "sorauﾛ1NfｷgﾉﾓﾉBｦKﾌﾘﾒoﾇﾂﾛrG81ﾋjWﾎﾕVncwﾌSｱ3pﾘﾋﾉhUS9Q76".to_owned(),
                amount: Some(Quantity::from(5_u64)),
            },
        },
        handle,
        proof: Some(ProofBlob {
            payload: norito::to_bytes(&envelope).expect("encode anchored-spend proof"),
            expiry_slot: Some(100),
        }),
        amount: Some(Quantity::from(5_u64)),
        amount_commitment: None,
    }
}
fn budget_key_for_replay_key(key: &AxtHandleReplayKey) -> AxtHandleBudgetKey {
    let mut handle = sample_asset_handle();
    handle.issuer_context.asset_dsid = key.asset_dsid;
    handle.issuer_context.asset_definition_incarnation = key.asset_definition_incarnation;
    handle.handle_era = key.handle_era;
    handle.target_lane = key.target_lane;
    handle.axt_binding = key.binding;
    AxtHandleBudgetKey::from_handle(&handle)
}
#[test]
fn unsigned_draft_cannot_decode_as_admission_handle() {
    let encoded = to_bytes(&sample_asset_handle_draft()).expect("encode unsigned draft");
    assert!(
        decode_from_bytes::<AssetHandle>(&encoded).is_err(),
        "the admission wire type must require its issuer context and signature"
    );
}
#[test]
fn finalized_spend_anchor_and_nonce_reject_every_absence_sentinel() {
    let network = test_network_id(b"finalized-anchor-network");
    let anchor = sample_finalized_spend_anchor(network, DataSpaceId::new(7), LaneId::new(2));
    assert_eq!(anchor.validate(), Ok(()));
    assert_ne!(anchor.digest_v1(), [0; 32]);
    assert_eq!(
        AxtSpendNonceV1::try_new([0; 32]),
        Err(AxtSpendNonceValidationErrorV1::Zero)
    );
    assert_eq!(
        AxtSpendNonceV1::try_new([0x77; 32])
            .expect("non-zero nonce")
            .validate(),
        Ok(())
    );

    let mut wrong_network = anchor;
    wrong_network.genesis_hash[0] ^= 1;
    assert_eq!(
        wrong_network.validate(),
        Err(AxtFinalizedSpendAnchorValidationErrorV1::NetworkGenesis)
    );
    let mut zero_height = anchor;
    zero_height.finalized_height = 0;
    assert_eq!(
        zero_height.validate(),
        Err(AxtFinalizedSpendAnchorValidationErrorV1::ZeroHeight)
    );
    let mutations = [
        (AxtFinalizedSpendAnchorFieldV1::LaneIncarnation, {
            let mut value = anchor;
            value.lane_incarnation = Hash::prehashed([0; Hash::LENGTH]);
            value
        }),
        (AxtFinalizedSpendAnchorFieldV1::BlockHeader, {
            let mut value = anchor;
            value.block_header_hash =
                HashOf::from_untyped_unchecked(Hash::prehashed([0; Hash::LENGTH]));
            value
        }),
        (AxtFinalizedSpendAnchorFieldV1::QuorumCertificate, {
            let mut value = anchor;
            value.quorum_certificate_digest = Hash::prehashed([0; Hash::LENGTH]);
            value
        }),
        (AxtFinalizedSpendAnchorFieldV1::Committee, {
            let mut value = anchor;
            value.committee_digest = Hash::prehashed([0; Hash::LENGTH]);
            value
        }),
        (AxtFinalizedSpendAnchorFieldV1::PreStateRoot, {
            let mut value = anchor;
            value.pre_state_root = Hash::prehashed([0; Hash::LENGTH]);
            value
        }),
        (AxtFinalizedSpendAnchorFieldV1::PostStateRoot, {
            let mut value = anchor;
            value.post_state_root = Hash::prehashed([0; Hash::LENGTH]);
            value
        }),
        (AxtFinalizedSpendAnchorFieldV1::TransactionSet, {
            let mut value = anchor;
            value.transaction_set_digest = Hash::prehashed([0; Hash::LENGTH]);
            value
        }),
        (AxtFinalizedSpendAnchorFieldV1::DaManifest, {
            let mut value = anchor;
            value.da_manifest_digest = Hash::prehashed([0; Hash::LENGTH]);
            value
        }),
    ];
    for (field, mutation) in mutations {
        assert_eq!(
            mutation.validate(),
            Err(AxtFinalizedSpendAnchorValidationErrorV1::ZeroBinding { field })
        );
    }
}
#[test]
fn anchored_spend_signature_binds_proof_amount_anchor_expiry_and_nonce() {
    let issuer = KeyPair::from_seed(vec![0x33; 32], Algorithm::Ed25519);
    let context = issuer_context(test_network_id(b"sequence-network"), DataSpaceId::new(7));
    let handle = sample_asset_handle_draft()
        .sign_by_issuer_v1(context, issuer.private_key())
        .expect("sign reusable handle");
    let anchor =
        sample_finalized_spend_anchor(context.network_id, context.asset_dsid, handle.target_lane);
    let nonce = AxtSpendNonceV1::try_new([0x77; 32]).expect("non-zero nonce");
    let draft = sample_anchored_spend_draft(handle, anchor);
    let envelope: AxtProofEnvelope =
        norito::decode_canonical(&draft.proof.as_ref().expect("proof").payload).expect("envelope");
    assert_ne!(
        envelope
            .fastpq_binding
            .as_ref()
            .expect("binding")
            .source_tx_commitment,
        hex::encode(anchor.transaction_set_digest.as_ref()),
        "per-execution identity is distinct from the ordered transaction-set digest",
    );
    let signed = draft
        .sign_by_issuer_v1(anchor, 100, nonce, issuer.private_key())
        .expect("sign exact anchored spend");
    assert_eq!(
        signed.verify_issuer_signatures_v1(context, anchor, issuer.public_key()),
        Ok(())
    );
    assert_eq!(
        signed.replay_key_v1(),
        AxtAnchoredSpendReplayKeyV1 {
            issuer_context: context,
            nonce,
        }
    );

    let mut changed_intent = signed.clone();
    changed_intent.draft.intent.op.to.push_str("-substitution");
    assert_eq!(
        changed_intent.verify_issuer_signatures_v1(context, anchor, issuer.public_key()),
        Err(AxtAnchoredSpendValidationErrorV1::SpendSignature)
    );
    let mut changed_proof = signed.clone();
    let proof = changed_proof
        .draft
        .proof
        .as_mut()
        .expect("signed spend carries proof");
    let mut envelope: AxtProofEnvelope =
        norito::decode_canonical(&proof.payload).expect("decode proof envelope");
    envelope.proof.push(0x11);
    proof.payload = norito::to_bytes(&envelope).expect("re-encode proof envelope");
    assert_eq!(
        changed_proof.verify_issuer_signatures_v1(context, anchor, issuer.public_key()),
        Err(AxtAnchoredSpendValidationErrorV1::SpendSignature)
    );
    let mut changed_amount = signed.clone();
    changed_amount.draft.amount = Some(Quantity::from(6_u64));
    assert_eq!(
        changed_amount.verify_issuer_signatures_v1(context, anchor, issuer.public_key()),
        Err(AxtAnchoredSpendValidationErrorV1::SpendSignature)
    );
    let mut changed_commitment = signed.clone();
    changed_commitment.draft.amount_commitment = Some([0x44; 32]);
    assert_eq!(
        changed_commitment.verify_issuer_signatures_v1(context, anchor, issuer.public_key()),
        Err(AxtAnchoredSpendValidationErrorV1::SpendSignature)
    );
    let mut changed_anchor = signed.clone();
    changed_anchor.authorization.anchor.committee_digest =
        Hash::new(b"substituted finalized committee");
    let substituted_anchor = changed_anchor.authorization.anchor;
    assert_eq!(
        changed_anchor.verify_issuer_signatures_v1(
            context,
            substituted_anchor,
            issuer.public_key()
        ),
        Err(AxtAnchoredSpendValidationErrorV1::SpendSignature)
    );
    let mut changed_expiry = signed.clone();
    changed_expiry.authorization.expiry_slot = 101;
    assert_eq!(
        changed_expiry.verify_issuer_signatures_v1(context, anchor, issuer.public_key()),
        Err(AxtAnchoredSpendValidationErrorV1::Expiry)
    );
    let mut changed_nonce = signed.clone();
    changed_nonce.authorization.nonce =
        AxtSpendNonceV1::try_new([0x78; 32]).expect("non-zero nonce");
    assert_eq!(
        changed_nonce.verify_issuer_signatures_v1(context, anchor, issuer.public_key()),
        Err(AxtAnchoredSpendValidationErrorV1::SpendSignature)
    );
    let impostor = KeyPair::from_seed(vec![0x34; 32], Algorithm::Ed25519);
    assert_eq!(
        signed.verify_issuer_signatures_v1(context, anchor, impostor.public_key()),
        Err(AxtAnchoredSpendValidationErrorV1::HandleSignature)
    );
    let mut wrong_authoritative_anchor = anchor;
    wrong_authoritative_anchor.finalized_height += 1;
    assert_eq!(
        signed.verify_issuer_signatures_v1(
            context,
            wrong_authoritative_anchor,
            issuer.public_key()
        ),
        Err(AxtAnchoredSpendValidationErrorV1::AnchorMismatch)
    );

    let retired_wire = norito::to_bytes(&signed.draft).expect("encode unsigned retired wire");
    assert!(
        norito::decode_from_bytes::<AxtAnchoredSpendV1>(&retired_wire).is_err(),
        "a pre-anchor spend draft must not decode as the admission wire"
    );
}
#[test]
fn handle_replay_key_scopes_identical_ticket_by_dataspace_and_asset_incarnation() {
    let handle = sample_asset_handle();
    let key_a = AxtHandleReplayKey::from_handle(DataSpaceId::new(7), &handle);
    let key_b = AxtHandleReplayKey::from_handle(DataSpaceId::new(8), &handle);
    assert_ne!(key_a, key_b);
    assert_eq!(key_a.binding, key_b.binding);
    assert_eq!(key_a.handle_era, key_b.handle_era);
    assert_eq!(key_a.sub_nonce, key_b.sub_nonce);
    assert_eq!(key_a.target_lane, key_b.target_lane);
    assert_eq!(
        key_a.asset_definition_incarnation,
        key_b.asset_definition_incarnation
    );

    let mut retired_handle = handle.clone();
    retired_handle.issuer_context.asset_definition_incarnation =
        test_asset_incarnation(b"retired-replay-incarnation");
    let retired_key = AxtHandleReplayKey::from_handle(DataSpaceId::new(7), &retired_handle);
    assert_ne!(key_a, retired_key);
    assert_eq!(key_a.binding, retired_key.binding);
    assert_eq!(key_a.handle_era, retired_key.handle_era);
    assert_eq!(key_a.sub_nonce, retired_key.sub_nonce);
    assert_eq!(key_a.target_lane, retired_key.target_lane);

    let encoded = to_bytes(&key_b).expect("encode dataspace-scoped replay key");
    let decoded: AxtHandleReplayKey =
        decode_from_bytes(&encoded).expect("decode dataspace-scoped replay key");
    assert_eq!(decoded, key_b);
    assert_eq!(decoded.asset_dsid, DataSpaceId::new(8));
}
#[test]
#[expect(
    clippy::too_many_lines,
    reason = "the commitment audit mutates every authenticated remote-spend runtime field in one table"
)]
fn remote_spend_intent_commitment_binds_every_runtime_field() {
    let dsid = DataSpaceId::new(7);
    let incarnation = test_asset_incarnation(b"remote-spend-current");
    let replay_key =
        AxtHandleReplayKey::from_parts(dsid, incarnation, [0xA5; 32], 11, 12, LaneId::new(3));
    let amount = Quantity::from(5_u64);
    let asset_definition = AssetDefinitionId::derive_from_components(
        DomainId::try_new("axt", "universal").expect("domain"),
        "rose".parse().expect("asset name"),
    );
    let expected = compute_remote_spend_intent_commitment_v1(
        replay_key,
        &asset_definition,
        "transfer",
        "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
        "sorauﾛ1NfｷgﾉﾓﾉBｦKﾌﾘﾒoﾇﾂﾛrG81ﾋjWﾎﾕVncwﾌSｱ3pﾘﾋﾉhUS9Q76",
        &amount,
    );
    assert_eq!(
        hex::encode(expected),
        "95d9bb334cb47eab805b80f1b18c94747c49b1d6bb51ec1e01c6cde688cca281",
        "V1 remote-spend commitment wire preimage changed"
    );
    assert_eq!(
        expected,
        compute_remote_spend_intent_commitment_v1(
            replay_key,
            &asset_definition,
            "transfer",
            "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
            "sorauﾛ1NfｷgﾉﾓﾉBｦKﾌﾘﾒoﾇﾂﾛrG81ﾋjWﾎﾕVncwﾌSｱ3pﾘﾋﾉhUS9Q76",
            &amount,
        )
    );
    let mutations = [
        compute_remote_spend_intent_commitment_v1(
            AxtHandleReplayKey::from_parts(dsid, incarnation, [0xA4; 32], 11, 12, LaneId::new(3)),
            &asset_definition,
            "transfer",
            "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
            "sorauﾛ1NfｷgﾉﾓﾉBｦKﾌﾘﾒoﾇﾂﾛrG81ﾋjWﾎﾕVncwﾌSｱ3pﾘﾋﾉhUS9Q76",
            &amount,
        ),
        compute_remote_spend_intent_commitment_v1(
            AxtHandleReplayKey::from_parts(
                DataSpaceId::new(8),
                incarnation,
                [0xA5; 32],
                11,
                12,
                LaneId::new(3),
            ),
            &asset_definition,
            "transfer",
            "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
            "sorauﾛ1NfｷgﾉﾓﾉBｦKﾌﾘﾒoﾇﾂﾛrG81ﾋjWﾎﾕVncwﾌSｱ3pﾘﾋﾉhUS9Q76",
            &amount,
        ),
        compute_remote_spend_intent_commitment_v1(
            AxtHandleReplayKey::from_parts(
                dsid,
                test_asset_incarnation(b"remote-spend-retired"),
                [0xA5; 32],
                11,
                12,
                LaneId::new(3),
            ),
            &asset_definition,
            "transfer",
            "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
            "sorauﾛ1NfｷgﾉﾓﾉBｦKﾌﾘﾒoﾇﾂﾛrG81ﾋjWﾎﾕVncwﾌSｱ3pﾘﾋﾉhUS9Q76",
            &amount,
        ),
        compute_remote_spend_intent_commitment_v1(
            AxtHandleReplayKey::from_parts(dsid, incarnation, [0xA5; 32], 10, 12, LaneId::new(3)),
            &asset_definition,
            "transfer",
            "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
            "sorauﾛ1NfｷgﾉﾓﾉBｦKﾌﾘﾒoﾇﾂﾛrG81ﾋjWﾎﾕVncwﾌSｱ3pﾘﾋﾉhUS9Q76",
            &amount,
        ),
        compute_remote_spend_intent_commitment_v1(
            AxtHandleReplayKey::from_parts(dsid, incarnation, [0xA5; 32], 11, 13, LaneId::new(3)),
            &asset_definition,
            "transfer",
            "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
            "sorauﾛ1NfｷgﾉﾓﾉBｦKﾌﾘﾒoﾇﾂﾛrG81ﾋjWﾎﾕVncwﾌSｱ3pﾘﾋﾉhUS9Q76",
            &amount,
        ),
        compute_remote_spend_intent_commitment_v1(
            AxtHandleReplayKey::from_parts(dsid, incarnation, [0xA5; 32], 11, 12, LaneId::new(4)),
            &asset_definition,
            "transfer",
            "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
            "sorauﾛ1NfｷgﾉﾓﾉBｦKﾌﾘﾒoﾇﾂﾛrG81ﾋjWﾎﾕVncwﾌSｱ3pﾘﾋﾉhUS9Q76",
            &amount,
        ),
        compute_remote_spend_intent_commitment_v1(
            replay_key,
            &AssetDefinitionId::derive_from_components(
                DomainId::try_new("axt", "universal").expect("domain"),
                "iris".parse().expect("asset name"),
            ),
            "transfer",
            "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
            "sorauﾛ1NfｷgﾉﾓﾉBｦKﾌﾘﾒoﾇﾂﾛrG81ﾋjWﾎﾕVncwﾌSｱ3pﾘﾋﾉhUS9Q76",
            &amount,
        ),
        compute_remote_spend_intent_commitment_v1(
            replay_key,
            &asset_definition,
            "mint",
            "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
            "sorauﾛ1NfｷgﾉﾓﾉBｦKﾌﾘﾒoﾇﾂﾛrG81ﾋjWﾎﾕVncwﾌSｱ3pﾘﾋﾉhUS9Q76",
            &amount,
        ),
        compute_remote_spend_intent_commitment_v1(
            replay_key,
            &asset_definition,
            "transfer",
            "sorauﾛ1NfｷgﾉﾓﾉBｦKﾌﾘﾒoﾇﾂﾛrG81ﾋjWﾎﾕVncwﾌSｱ3pﾘﾋﾉhUS9Q76",
            "sorauﾛ1NfｷgﾉﾓﾉBｦKﾌﾘﾒoﾇﾂﾛrG81ﾋjWﾎﾕVncwﾌSｱ3pﾘﾋﾉhUS9Q76",
            &amount,
        ),
        compute_remote_spend_intent_commitment_v1(
            replay_key,
            &asset_definition,
            "transfer",
            "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
            "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
            &amount,
        ),
        compute_remote_spend_intent_commitment_v1(
            replay_key,
            &asset_definition,
            "transfer",
            "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
            "sorauﾛ1NfｷgﾉﾓﾉBｦKﾌﾘﾒoﾇﾂﾛrG81ﾋjWﾎﾕVncwﾌSｱ3pﾘﾋﾉhUS9Q76",
            &Quantity::from(6_u64),
        ),
    ];
    assert!(
        mutations
            .into_iter()
            .all(|commitment| commitment != expected)
    );
}
#[test]
fn remote_spend_claim_roundtrips_and_matches_component_commitment() {
    let asset_definition = AssetDefinitionId::derive_from_components(
        DomainId::try_new("axt", "universal").expect("domain"),
        "rose".parse().expect("asset name"),
    );
    let replay_key = AxtHandleReplayKey::from_parts(
        DataSpaceId::new(7),
        test_asset_incarnation(b"remote-spend-claim"),
        [0xA5; 32],
        11,
        12,
        LaneId::new(3),
    );
    let claim = AxtRemoteSpendClaimV1::new(
        replay_key,
        asset_definition,
        "transfer",
        "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
        "sorauﾛ1NfｷgﾉﾓﾉBｦKﾌﾘﾒoﾇﾂﾛrG81ﾋjWﾎﾕVncwﾌSｱ3pﾘﾋﾉhUS9Q76",
        Quantity::from(5_u64),
    );
    let encoded = to_bytes(&claim).expect("encode remote-spend claim");
    let decoded: AxtRemoteSpendClaimV1 =
        decode_from_bytes(&encoded).expect("decode remote-spend claim");
    assert_eq!(decoded, claim);
    assert_eq!(
        compute_remote_spend_claim_commitment_v1(&claim),
        compute_remote_spend_intent_commitment_v1(
            claim.handle_replay_key,
            &claim.asset_definition_id,
            &claim.kind,
            &claim.from,
            &claim.to,
            &claim.effective_amount,
        )
    );
}
#[test]
#[expect(
    clippy::too_many_lines,
    reason = "the incarnation audit keeps its canonical vector, identity mutations, and zero rejection together"
)]
fn asset_incarnation_is_nonzero_canonical_and_binds_registration_identity() {
    let golden_network = NetworkId::from_genesis_hash(
        HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0x11; 32])),
    );
    let golden_asset =
        AssetDefinitionId::from_uuid_bytes([0, 0, 0, 0, 0, 0, 0x40, 0, 0x80, 0, 0, 0, 0, 0, 0, 1])
            .expect("golden asset identifier is canonical UUIDv4");
    let golden_header = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0x33; 32]));
    let golden_execution = Hash::prehashed([0x55; 32]);
    let golden = AxtAssetIncarnationV1::derive(
        &golden_network,
        &golden_asset,
        &golden_header,
        &golden_execution,
        0x0102_0304_0506_0708,
    );
    assert_eq!(
        hex::encode(golden.as_bytes()),
        "a744e8a34aacfa4cdc9ae4407b88d3710c594bf2f5cbf7c68308353e33b4992d",
        "V1 domain/chunk ordering and big-endian ordinal are wire commitments"
    );

    let network = test_network_id(b"asset-incarnation-network");
    let asset = test_asset_definition_id();
    let header = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
        b"asset-incarnation-registration-header",
    ));
    let execution_identity = Hash::new(b"asset-incarnation-registration-execution");
    let incarnation =
        AxtAssetIncarnationV1::derive(&network, &asset, &header, &execution_identity, 3);
    assert_eq!(incarnation.validate(), Ok(()));
    assert!(incarnation.as_bytes().iter().any(|byte| *byte != 0));
    assert_eq!(
        AxtAssetIncarnationV1::try_from_bytes(*incarnation.as_bytes()),
        Ok(incarnation)
    );
    assert_eq!(
        AxtAssetIncarnationV1::try_from_bytes([0; Hash::LENGTH]),
        Err(AxtAssetIncarnationValidationError::Zero)
    );
    assert_eq!(
        AxtAssetIncarnationV1::try_from_bytes(Hash::prehashed([0; Hash::LENGTH]).into()),
        Err(AxtAssetIncarnationValidationError::Zero),
        "the hash marker alone is still the logical absence sentinel"
    );
    let mut invalid_marker = [0; Hash::LENGTH];
    invalid_marker[0] = 1;
    assert_eq!(
        AxtAssetIncarnationV1::try_from_bytes(invalid_marker),
        Err(AxtAssetIncarnationValidationError::InvalidHashMarker)
    );

    let other_network = test_network_id(b"other-asset-incarnation-network");
    assert_ne!(
        incarnation,
        AxtAssetIncarnationV1::derive(&other_network, &asset, &header, &execution_identity, 3,)
    );
    let other_asset = AssetDefinitionId::derive_from_components(
        DomainId::try_new("axt", "universal").expect("domain"),
        "iris".parse().expect("asset name"),
    );
    assert_ne!(
        incarnation,
        AxtAssetIncarnationV1::derive(&network, &other_asset, &header, &execution_identity, 3,)
    );
    let other_header = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
        b"other-asset-incarnation-registration-header",
    ));
    assert_ne!(
        incarnation,
        AxtAssetIncarnationV1::derive(&network, &asset, &other_header, &execution_identity, 3,)
    );
    let other_execution = Hash::new(b"other-asset-incarnation-registration-execution");
    assert_ne!(
        incarnation,
        AxtAssetIncarnationV1::derive(&network, &asset, &header, &other_execution, 3)
    );
    assert_ne!(
        incarnation,
        AxtAssetIncarnationV1::derive(&network, &asset, &header, &execution_identity, 4)
    );

    let encoded = to_bytes(&incarnation).expect("encode asset incarnation");
    assert_eq!(
        decode_from_bytes::<AxtAssetIncarnationV1>(&encoded).expect("decode asset incarnation"),
        incarnation
    );

    {
        let context = issuer_context(network, DataSpaceId::new(7));
        let mut value = norito::json::to_value(&context).expect("encode issuer context JSON");
        let mut logical_zero = norito::json::to_value(&context.asset_definition_incarnation)
            .expect("encode incarnation JSON");
        logical_zero
            .as_array_mut()
            .expect("transparent incarnation JSON tuple")[0] =
            norito::json::to_value(&Hash::prehashed([0; Hash::LENGTH]))
                .expect("encode logical-zero hash");
        value
            .as_object_mut()
            .expect("issuer context JSON object")
            .insert("asset_definition_incarnation".to_owned(), logical_zero);
        let decoded = norito::json::from_value::<AxtHandleIssuerContextV1>(value)
            .expect("the typed hash marker is syntactically valid");
        assert_eq!(
            decoded.validate(),
            Err(AxtAssetIncarnationValidationError::Zero),
            "contextual validation must reject the logical-zero token"
        );
    }
}
#[test]
#[expect(
    clippy::too_many_lines,
    reason = "the signature audit mutates every policy field and network binding in one fixture"
)]
fn asset_handle_issuer_signature_binds_every_policy_field_and_network() {
    let issuer = KeyPair::from_seed(vec![0x11; 32], Algorithm::Ed25519);
    let impostor = KeyPair::from_seed(vec![0x22; 32], Algorithm::Ed25519);
    let dsid = DataSpaceId::new(7);
    let context = issuer_context(test_network_id(b"iroha-test-network"), dsid);
    let signed = sample_asset_handle_draft()
        .sign_by_issuer_v1(context, issuer.private_key())
        .expect("sign fixture handle");
    assert!(
        signed
            .verify_issuer_signature_v1(context, issuer.public_key())
            .is_ok()
    );
    assert!(
        signed
            .verify_issuer_signature_v1(context, impostor.public_key())
            .is_err(),
        "a forged issuer must not authenticate"
    );
    let mut wrong_contexts = Vec::new();
    let mut wrong = context;
    wrong.network_id = test_network_id(b"other-network");
    wrong_contexts.push(wrong);
    let mut wrong = context;
    wrong.asset_dsid = DataSpaceId::new(8);
    wrong_contexts.push(wrong);
    let mut wrong = context;
    wrong.asset_definition_incarnation = AxtAssetIncarnationV1::derive(
        &context.network_id,
        &test_asset_definition_id(),
        &HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
            b"other-asset-incarnation-header",
        )),
        &Hash::new(b"other-asset-incarnation-execution"),
        0,
    );
    wrong_contexts.push(wrong);
    let mut wrong = context;
    wrong.issuer = UniversalAccountId::from_hash(Hash::new(b"other-issuer"));
    wrong_contexts.push(wrong);
    let mut wrong = context;
    wrong.issuer_manifest_root[0] ^= 1;
    wrong_contexts.push(wrong);
    let mut wrong = context;
    wrong.code_root[0] ^= 1;
    wrong_contexts.push(wrong);
    let mut wrong = context;
    wrong.abi_version += 1;
    wrong_contexts.push(wrong);
    let mut wrong = context;
    wrong.abi_hash[0] ^= 1;
    wrong_contexts.push(wrong);
    for wrong in wrong_contexts {
        assert!(
            signed
                .verify_issuer_signature_v1(wrong, issuer.public_key())
                .is_err(),
            "issuer signatures must bind the exact external admission context"
        );
    }
    let mut altered = Vec::new();
    let mut handle = signed.clone();
    handle.asset_definition_id = AssetDefinitionId::derive_from_components(
        DomainId::try_new("axt", "universal").expect("domain"),
        "iris".parse().expect("asset name"),
    );
    altered.push(handle);
    let mut handle = signed.clone();
    handle.scope.push("mint".into());
    altered.push(handle);
    let mut handle = signed.clone();
    handle.subject.account.push_str("-altered");
    altered.push(handle);
    let mut handle = signed.clone();
    handle.budget.remaining = Quantity::from(51_u64);
    altered.push(handle);
    let mut handle = signed.clone();
    handle.handle_era += 1;
    altered.push(handle);
    let mut handle = signed.clone();
    handle.sub_nonce += 1;
    altered.push(handle);
    let mut handle = signed.clone();
    handle.group_binding.epoch_id += 1;
    altered.push(handle);
    let mut handle = signed.clone();
    handle.target_lane = LaneId::new(3);
    altered.push(handle);
    let mut handle = signed.clone();
    handle.axt_binding = AxtBinding::new([0xA4; 32]);
    altered.push(handle);
    let mut handle = signed.clone();
    handle.manifest_view_root[0] ^= 1;
    altered.push(handle);
    let mut handle = signed.clone();
    handle.expiry_slot += 1;
    altered.push(handle);
    let mut handle = signed.clone();
    handle.max_clock_skew_ms = Some(26);
    altered.push(handle);
    for altered in altered {
        assert!(
            altered
                .verify_issuer_signature_v1(context, issuer.public_key())
                .is_err(),
            "altering an issuer-bound handle field must invalidate the signature"
        );
    }
}
#[test]
fn handle_budget_key_omits_only_counter_and_signature() {
    let signed = sample_asset_handle();
    let payload = signed.draft().issuer_payload_v1(signed.issuer_context);
    let expected = AxtHandleBudgetKey::from_handle(&signed);
    assert_eq!(
        expected,
        AxtHandleBudgetKey::from_issuer_payload_v1(&payload)
    );
    assert_eq!(expected.asset_dsid(), signed.issuer_context.asset_dsid);
    assert_eq!(expected.target_lane(), signed.target_lane);
    assert_eq!(expected.authorization_generation(), signed.handle_era);

    let mut next_counter = payload.clone();
    next_counter.next_handle_counter = next_counter.next_handle_counter.saturating_add(1);
    assert_eq!(
        expected,
        AxtHandleBudgetKey::from_issuer_payload_v1(&next_counter),
        "sequential sub-nonces must share one cumulative family"
    );
    let mut other_signature = signed.clone();
    other_signature.issuer_signature = Signature::from_bytes(&[0xA7; 64]);
    assert_eq!(
        expected,
        AxtHandleBudgetKey::from_handle(&other_signature),
        "signature encoding authenticates but does not identify the family"
    );

    let mut mutations = Vec::new();
    let mut changed = payload.clone();
    changed.context.network_id = test_network_id(b"other-budget-network");
    mutations.push(changed);
    let mut changed = payload.clone();
    changed.context.asset_definition_incarnation = AxtAssetIncarnationV1::derive(
        &changed.context.network_id,
        &changed.asset_definition_id,
        &HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
            b"other-budget-asset-registration",
        )),
        &Hash::new(b"other-budget-asset-registration-execution"),
        0,
    );
    mutations.push(changed);
    let mut changed = payload.clone();
    changed.asset_definition_id = AssetDefinitionId::derive_from_components(
        DomainId::try_new("axt", "universal").expect("domain"),
        "iris".parse().expect("asset name"),
    );
    mutations.push(changed);
    let mut changed = payload.clone();
    changed.scope.push("mint".to_owned());
    mutations.push(changed);
    let mut changed = payload.clone();
    changed.subject.origin_dsid = Some(DataSpaceId::new(99));
    mutations.push(changed);
    let mut changed = payload.clone();
    changed.budget.remaining = Quantity::from(51_u64);
    mutations.push(changed);
    let mut changed = payload.clone();
    changed.active_handle_era = changed.active_handle_era.saturating_add(1);
    mutations.push(changed);
    let mut changed = payload.clone();
    changed.group_binding.epoch_id = changed.group_binding.epoch_id.saturating_add(1);
    mutations.push(changed);
    let mut changed = payload.clone();
    changed.target_lane = LaneId::new(changed.target_lane.as_u32().saturating_add(1));
    mutations.push(changed);
    let mut changed = payload.clone();
    changed.axt_binding = AxtBinding::new([0xA4; 32]);
    mutations.push(changed);
    let mut changed = payload.clone();
    changed.manifest_view_root[0] ^= 1;
    mutations.push(changed);
    let mut changed = payload.clone();
    changed.expiry_slot = changed.expiry_slot.saturating_add(1);
    mutations.push(changed);
    let mut changed = payload;
    changed.max_clock_skew_ms = Some(26);
    mutations.push(changed);
    for changed in mutations {
        assert_ne!(
            expected,
            AxtHandleBudgetKey::from_issuer_payload_v1(&changed),
            "every other issuer-signed field must identify the budget family"
        );
    }
}
#[test]
#[expect(
    clippy::too_many_lines,
    reason = "the budget audit keeps atomic limit transitions and canonical roundtrip checks together"
)]
fn handle_budget_record_enforces_limits_atomically_and_roundtrips() {
    let mut handle = sample_asset_handle();
    handle.budget.remaining = Quantity::from(50_u64);
    handle.budget.per_use = Some(Quantity::from(10_u64));
    let key = AxtHandleBudgetKey::from_handle(&handle);
    let mut record = AxtHandleBudgetRecord::empty();
    let empty = record.clone();
    assert_eq!(
        record.try_consume(&key, &Quantity::zero(), 20),
        Err(AxtHandleBudgetConsumeError::ZeroAmount)
    );
    assert_eq!(record, empty, "failed consumption must be atomic");
    record
        .try_consume(&key, &Quantity::from(6_u64), 20)
        .expect("first consumption");
    record
        .try_consume(&key, &Quantity::from(4_u64), 10)
        .expect("exact per-use aggregate cap");
    record
        .validate_for_key(&key)
        .expect("valid persisted record");
    assert_eq!(record.consumed(), &Quantity::from(10_u64));
    assert_eq!(record.retain_until_slot(), 20, "retention is monotonic");
    let at_limit = record.clone();
    assert_eq!(
        record.try_consume(&key, &Quantity::from(1_u64), 30),
        Err(AxtHandleBudgetConsumeError::PerUseExceeded)
    );
    assert_eq!(
        record, at_limit,
        "limit rejection must not mutate retention"
    );

    handle.budget.per_use = None;
    let remaining_key = AxtHandleBudgetKey::from_handle(&handle);
    let mut remaining = AxtHandleBudgetRecord::empty();
    remaining
        .try_consume(&remaining_key, &Quantity::from(50_u64), 40)
        .expect("exact remaining cap");
    let at_remaining = remaining.clone();
    assert_eq!(
        remaining.try_consume(&remaining_key, &Quantity::from(1_u64), 50),
        Err(AxtHandleBudgetConsumeError::RemainingExceeded)
    );
    assert_eq!(remaining, at_remaining);

    let mut fractional_handle = handle.clone();
    fractional_handle.budget.remaining = Quantity::from(100_u64);
    let fractional_key = AxtHandleBudgetKey::from_handle(&fractional_handle);
    let mut fractional = AxtHandleBudgetRecord::empty();
    fractional
        .try_consume(
            &fractional_key,
            &"0.5".parse().expect("fractional quantity"),
            60,
        )
        .expect("canonical exact decimals need not have equal scales");

    let mut maximum_bytes = vec![0xFF_u8; 63];
    maximum_bytes.push(0x7F);
    let maximum = Quantity::from_canonical_numeric(Numeric::new(
        BigInt::from_twos_bytes(&maximum_bytes).expect("signed maximum"),
        0,
    ))
    .expect("signed maximum is a quantity");
    let mut overflow_handle = handle;
    overflow_handle.budget.remaining = maximum.clone();
    let overflow_key = AxtHandleBudgetKey::from_handle(&overflow_handle);
    let mut overflow = AxtHandleBudgetRecord {
        consumed: maximum,
        retain_until_slot: 70,
    };
    let before_overflow = overflow.clone();
    assert_eq!(
        overflow.try_consume(&overflow_key, &Quantity::from(1_u64), 80),
        Err(AxtHandleBudgetConsumeError::Arithmetic(
            NumericOperationError::MantissaOverflow
        ))
    );
    assert_eq!(overflow, before_overflow);

    assert_eq!(
        AxtHandleBudgetRecord::empty().validate_for_key(&key),
        Err(AxtHandleBudgetConsumeError::ZeroAmount),
        "committed state must not contain empty accumulator records"
    );
    let over_remaining = AxtHandleBudgetRecord {
        consumed: Quantity::from(51_u64),
        retain_until_slot: 90,
    };
    assert_eq!(
        over_remaining.validate_for_key(&remaining_key),
        Err(AxtHandleBudgetConsumeError::RemainingExceeded)
    );
    let over_per_use = AxtHandleBudgetRecord {
        consumed: Quantity::from(11_u64),
        retain_until_slot: 90,
    };
    assert_eq!(
        over_per_use.validate_for_key(&key),
        Err(AxtHandleBudgetConsumeError::PerUseExceeded)
    );

    let key_bytes = to_bytes(&key).expect("encode budget key");
    assert_eq!(
        decode_from_bytes::<AxtHandleBudgetKey>(&key_bytes).expect("decode budget key"),
        key
    );
    let record_bytes = to_bytes(&record).expect("encode budget record");
    assert_eq!(
        decode_from_bytes::<AxtHandleBudgetRecord>(&record_bytes).expect("decode budget record"),
        record
    );
}
#[test]
#[expect(
    clippy::too_many_lines,
    reason = "the permanent counter audit keeps boundary, overflow, generation, and canonical checks together"
)]
fn handle_counter_record_is_permanent_exact_and_checked() {
    let mut record = AxtHandleCounterRecord::initial(4);
    assert_eq!(record.next(), 1);
    assert_eq!(record.authorization_generation(), 4);
    assert_eq!(record.validate(), Ok(()));
    let installed = AxtHandleCounterRecord::try_from_parts(7, 9)
        .expect("validated authoritative setup ratchet");
    assert_eq!(installed.next(), 7);
    assert_eq!(installed.authorization_generation(), 9);
    assert_eq!(
        AxtHandleCounterRecord::try_from_parts(0, 9),
        Err(AxtHandleCounterError::ZeroNextCounter)
    );

    let before = record;
    assert_eq!(
        record.try_advance(4, 2),
        Err(AxtHandleCounterError::SubNonceMismatch {
            expected: 1,
            actual: 2,
        })
    );
    assert_eq!(record, before, "future-value rejection must be atomic");
    assert_eq!(
        record.try_advance(3, 1),
        Err(AxtHandleCounterError::AuthorizationGenerationMismatch {
            expected: 4,
            actual: 3,
        })
    );
    assert_eq!(record, before, "generation rejection must be atomic");
    assert_eq!(record.try_advance(4, 1), Ok(()));
    assert_eq!(record.next(), 2);
    assert_eq!(record.authorization_generation(), 4);
    assert_eq!(
        record.try_revoke_for_policy_transition(9),
        Ok(()),
        "policy transition must revoke both signed dimensions"
    );
    assert_eq!(record.next(), 3);
    assert_eq!(record.authorization_generation(), 9);
    let mut incremented =
        AxtHandleCounterRecord::try_from_parts(4, 9).expect("valid transition fixture");
    incremented
        .try_revoke_for_policy_transition(3)
        .expect("lower derived era still advances the generation");
    assert_eq!(incremented.next(), 5);
    assert_eq!(incremented.authorization_generation(), 10);
    assert_eq!(
        AxtHandleCounterRecord::initial(0).authorization_generation(),
        1,
        "zero is reserved for an absent/inactive policy"
    );
    assert_eq!(
        AxtHandleCounterRecord::try_from_parts(1, 0),
        Err(AxtHandleCounterError::ZeroAuthorizationGeneration)
    );
    assert_eq!(
        record.try_advance(9, 1),
        Err(AxtHandleCounterError::SubNonceMismatch {
            expected: 3,
            actual: 1,
        })
    );

    let invalid = AxtHandleCounterRecord {
        next: 0,
        authorization_generation: 9,
    };
    assert_eq!(
        invalid.validate(),
        Err(AxtHandleCounterError::ZeroNextCounter)
    );
    let inactive = AxtHandleCounterRecord {
        next: 1,
        authorization_generation: 0,
    };
    assert_eq!(
        inactive.validate(),
        Err(AxtHandleCounterError::ZeroAuthorizationGeneration)
    );
    let mut exhausted = AxtHandleCounterRecord {
        next: u64::MAX,
        authorization_generation: 9,
    };
    let before = exhausted;
    assert_eq!(
        exhausted.try_advance(9, u64::MAX),
        Err(AxtHandleCounterError::CounterExhausted)
    );
    assert_eq!(exhausted, before, "overflow rejection must be atomic");
    assert_eq!(
        exhausted.try_revoke_for_policy_transition(10),
        Err(AxtHandleCounterError::CounterExhausted)
    );
    assert_eq!(exhausted, before, "revocation overflow must be atomic");
    let mut generation_exhausted = AxtHandleCounterRecord {
        next: 3,
        authorization_generation: u64::MAX,
    };
    let before = generation_exhausted;
    assert_eq!(
        generation_exhausted.try_revoke_for_policy_transition(u64::MAX),
        Err(AxtHandleCounterError::AuthorizationGenerationExhausted)
    );
    assert_eq!(
        generation_exhausted, before,
        "generation overflow must leave both dimensions unchanged"
    );

    let encoded = to_bytes(&record).expect("encode handle counter ratchet");
    assert_eq!(
        decode_from_bytes::<AxtHandleCounterRecord>(&encoded)
            .expect("decode handle counter ratchet"),
        record
    );

    {
        let value = norito::json::to_value(&record).expect("encode counter JSON");
        assert_eq!(
            norito::json::from_value::<AxtHandleCounterRecord>(value).expect("decode counter JSON"),
            record
        );
    }
}

#[test]
fn handle_budget_key_json_storage_key_roundtrips() {
    let key = AxtHandleBudgetKey::from_handle(&sample_asset_handle());
    let mut encoded = String::new();
    key.encode_json_key(&mut encoded);
    let mut parser = norito::json::Parser::new(&encoded);
    let raw_key = parser.parse_string().expect("parse JSON storage key");
    assert_eq!(
        AxtHandleBudgetKey::decode_json_key(&raw_key).expect("decode JSON storage key"),
        key
    );
    assert!(
        AxtHandleBudgetKey::decode_json_key(&(raw_key.clone() + " ")).is_err(),
        "non-canonical whitespace must not alias the canonical snapshot key"
    );
    assert!(
        AxtHandleBudgetKey::decode_json_key(&(raw_key + "true")).is_err(),
        "trailing JSON must not alias the canonical snapshot key"
    );
}
#[test]
fn handle_sequence_accepts_only_exact_checked_progression() {
    let mut policy = AxtPolicyEntry {
        manifest_root: [0x5A; 32],
        target_lane: LaneId::new(2),
        active_handle_era: 9,
        next_handle_counter: 3,
        current_slot: 1,
    };
    let mut handle = sample_asset_handle();
    assert_eq!(next_axt_handle_sub_nonce(&policy, &handle), Ok(4));
    policy.next_handle_counter = 4;
    handle.sub_nonce = 4;
    assert_eq!(next_axt_handle_sub_nonce(&policy, &handle), Ok(5));
    handle.sub_nonce = 3;
    assert!(matches!(
        next_axt_handle_sub_nonce(&policy, &handle),
        Err(AxtHandleSequenceError::SubNonceMismatch {
            expected: 4,
            actual: 3
        })
    ));
    handle.sub_nonce = u64::MAX;
    assert!(matches!(
        next_axt_handle_sub_nonce(&policy, &handle),
        Err(AxtHandleSequenceError::SubNonceMismatch { .. })
    ));
    handle.sub_nonce = policy.next_handle_counter;
    handle.handle_era = u64::MAX;
    assert!(matches!(
        next_axt_handle_sub_nonce(&policy, &handle),
        Err(AxtHandleSequenceError::EraMismatch {
            expected: 9,
            actual: u64::MAX
        })
    ));
    policy.active_handle_era = u64::MAX;
    policy.next_handle_counter = u64::MAX;
    handle.handle_era = u64::MAX;
    handle.sub_nonce = u64::MAX;
    assert_eq!(
        next_axt_handle_sub_nonce(&policy, &handle),
        Err(AxtHandleSequenceError::CounterExhausted)
    );
}
fn sample_fastpq_binding(dsid: DataSpaceId) -> AxtFastpqBinding {
    AxtFastpqBinding {
        parameter: "fastpq-state-transition-stark-v1".to_string(),
        source_dsid: dsid.as_u64(),
        source_dataspace: format!("test-dataspace-{}", dsid.as_u64()),
        source_receipt_id: format!("receipt-{}", dsid.as_u64()),
        source_tx_commitment: "aa".repeat(32),
        claim_type: "authorization".to_string(),
        claim_digest: "bb".repeat(32),
        witness_commitment: "cc".repeat(32),
        policy_commitment: "dd".repeat(32),
        verified_effect_type: "test_effect".to_string(),
        corridor: "test-corridor".to_string(),
        verifier_id: "fastpq".to_string(),
        verifier_version: "v1".to_string(),
        target_dsids: vec![dsid.as_u64()],
        effect_binding: None,
        remote_spend_intent_commitments: Vec::new(),
    }
}
#[test]
#[expect(
    clippy::too_many_lines,
    reason = "the V1 binary audit enumerates every retired layout that previously defaulted a required field"
)]
fn axt_v1_rejects_pre_release_binary_layouts_with_defaulted_fields() {
    #[derive(Encode, norito::NoritoSchema)]
    #[norito_schema(
        name = "test::iroha_data_model::nexus::axt::PreReleaseDescriptor",
        frame = "iroha_data_model::nexus::axt::AxtDescriptor"
    )]
    struct PreReleaseDescriptor {
        dsids: Vec<DataSpaceId>,
    }
    #[derive(Encode, norito::NoritoSchema)]
    #[norito_schema(
        name = "test::iroha_data_model::nexus::axt::PreReleaseProofEnvelope",
        frame = "iroha_data_model::nexus::axt::AxtProofEnvelope"
    )]
    struct PreReleaseProofEnvelope {
        dsid: DataSpaceId,
        manifest_root: [u8; 32],
        da_commitment: Option<[u8; 32]>,
        proof: Vec<u8>,
        fastpq_binding: Option<AxtFastpqBinding>,
    }
    #[derive(Encode, norito::NoritoSchema)]
    #[norito_schema(
        name = "test::iroha_data_model::nexus::axt::PreReleaseProofBlob",
        frame = "iroha_data_model::nexus::axt::ProofBlob"
    )]
    struct PreReleaseProofBlob {
        payload: Vec<u8>,
    }
    #[derive(Encode, norito::NoritoSchema)]
    #[norito_schema(
        name = "test::iroha_data_model::nexus::axt::PreReleaseHandleBudget",
        frame = "iroha_data_model::nexus::axt::HandleBudget"
    )]
    struct PreReleaseHandleBudget {
        remaining: Quantity,
    }
    #[derive(Encode, norito::NoritoSchema)]
    #[norito_schema(
        name = "test::iroha_data_model::nexus::axt::PreReleaseHandleSubject",
        frame = "iroha_data_model::nexus::axt::HandleSubject"
    )]
    struct PreReleaseHandleSubject {
        account: String,
    }
    #[derive(Encode, norito::NoritoSchema)]
    #[norito_schema(
        name = "test::iroha_data_model::nexus::axt::PreReleaseAssetHandleDraft",
        frame = "iroha_data_model::nexus::axt::AssetHandleDraft"
    )]
    struct PreReleaseAssetHandleDraft {
        scope: Vec<String>,
        subject: HandleSubject,
        budget: HandleBudget,
        handle_era: u64,
        sub_nonce: u64,
        group_binding: GroupBinding,
        target_lane: LaneId,
        axt_binding: AxtBinding,
        manifest_view_root: [u8; 32],
        expiry_slot: u64,
        max_clock_skew_ms: Option<u32>,
    }
    #[derive(Encode, norito::NoritoSchema)]
    #[norito_schema(
        name = "test::iroha_data_model::nexus::axt::PreReleaseSpendOp",
        frame = "iroha_data_model::nexus::axt::SpendOp"
    )]
    struct PreReleaseSpendOp {
        kind: String,
        from: String,
        to: String,
        amount: Option<Quantity>,
    }

    let dsid = DataSpaceId::new(19);
    let descriptor = to_bytes(&PreReleaseDescriptor { dsids: vec![dsid] })
        .expect("encode pre-release AXT descriptor");
    assert_eq!(
        norito::schema::identity::frame_hash::<PreReleaseDescriptor>(),
        norito::schema::identity::frame_hash::<AxtDescriptor>(),
        "malformed fixture must use the production frame identity"
    );
    assert_eq!(
        norito::core::Header::read(descriptor.as_slice())
            .expect("read malformed fixture header")
            .schema,
        norito::schema::identity::frame_hash::<AxtDescriptor>(),
        "malformed fixture header must reach the production decoder"
    );
    assert!(
        decode_from_bytes::<AxtDescriptor>(&descriptor).is_err(),
        "the V1 descriptor must require its exact touch collection"
    );

    let envelope = to_bytes(&PreReleaseProofEnvelope {
        dsid,
        manifest_root: [0xA5; 32],
        da_commitment: None,
        proof: vec![0xC3],
        fastpq_binding: Some(sample_fastpq_binding(dsid)),
    })
    .expect("encode pre-release AXT proof envelope");
    assert_eq!(
        norito::schema::identity::frame_hash::<PreReleaseProofEnvelope>(),
        norito::schema::identity::frame_hash::<AxtProofEnvelope>(),
        "malformed fixture must use the production frame identity"
    );
    assert_eq!(
        norito::core::Header::read(envelope.as_slice())
            .expect("read malformed fixture header")
            .schema,
        norito::schema::identity::frame_hash::<AxtProofEnvelope>(),
        "malformed fixture header must reach the production decoder"
    );
    assert!(
        decode_from_bytes::<AxtProofEnvelope>(&envelope).is_err(),
        "the V1 proof envelope must require amount and commitment slots"
    );

    let shortened = to_bytes(&PreReleaseProofBlob {
        payload: vec![0xC5],
    })
    .expect("encode pre-release proof blob");
    assert_eq!(
        norito::schema::identity::frame_hash::<PreReleaseProofBlob>(),
        norito::schema::identity::frame_hash::<ProofBlob>(),
        "malformed fixture must use the production frame identity"
    );
    assert_eq!(
        norito::core::Header::read(shortened.as_slice())
            .expect("read malformed fixture header")
            .schema,
        norito::schema::identity::frame_hash::<ProofBlob>(),
        "malformed fixture header must reach the production decoder"
    );
    assert!(decode_from_bytes::<ProofBlob>(&shortened).is_err());
    let shortened = to_bytes(&PreReleaseHandleBudget {
        remaining: Quantity::from(5_u64),
    })
    .expect("encode pre-release handle budget");
    assert_eq!(
        norito::schema::identity::frame_hash::<PreReleaseHandleBudget>(),
        norito::schema::identity::frame_hash::<HandleBudget>(),
        "malformed fixture must use the production frame identity"
    );
    assert_eq!(
        norito::core::Header::read(shortened.as_slice())
            .expect("read malformed fixture header")
            .schema,
        norito::schema::identity::frame_hash::<HandleBudget>(),
        "malformed fixture header must reach the production decoder"
    );
    assert!(decode_from_bytes::<HandleBudget>(&shortened).is_err());
    let shortened = to_bytes(&PreReleaseHandleSubject {
        account: "sorau fixture".to_owned(),
    })
    .expect("encode pre-release handle subject");
    assert_eq!(
        norito::schema::identity::frame_hash::<PreReleaseHandleSubject>(),
        norito::schema::identity::frame_hash::<HandleSubject>(),
        "malformed fixture must use the production frame identity"
    );
    assert_eq!(
        norito::core::Header::read(shortened.as_slice())
            .expect("read malformed fixture header")
            .schema,
        norito::schema::identity::frame_hash::<HandleSubject>(),
        "malformed fixture header must reach the production decoder"
    );
    assert!(decode_from_bytes::<HandleSubject>(&shortened).is_err());
    let draft = sample_asset_handle_draft();
    let shortened = to_bytes(&PreReleaseAssetHandleDraft {
        scope: draft.scope,
        subject: draft.subject,
        budget: draft.budget,
        handle_era: draft.handle_era,
        sub_nonce: draft.sub_nonce,
        group_binding: draft.group_binding,
        target_lane: draft.target_lane,
        axt_binding: draft.axt_binding,
        manifest_view_root: draft.manifest_view_root,
        expiry_slot: draft.expiry_slot,
        max_clock_skew_ms: draft.max_clock_skew_ms,
    })
    .expect("encode pre-asset-binding handle draft");
    assert_eq!(
        norito::schema::identity::frame_hash::<PreReleaseAssetHandleDraft>(),
        norito::schema::identity::frame_hash::<AssetHandleDraft>(),
        "malformed fixture must use the production frame identity"
    );
    assert_eq!(
        norito::core::Header::read(shortened.as_slice())
            .expect("read malformed fixture header")
            .schema,
        norito::schema::identity::frame_hash::<AssetHandleDraft>(),
        "malformed fixture header must reach the production decoder"
    );
    assert!(
        decode_from_bytes::<AssetHandleDraft>(&shortened).is_err(),
        "the V1 handle draft must require its issuer-signed asset definition"
    );
    let shortened = to_bytes(&PreReleaseSpendOp {
        kind: "transfer".to_owned(),
        from: "sorau source".to_owned(),
        to: "sorau destination".to_owned(),
        amount: None,
    })
    .expect("encode pre-asset-binding spend operation");
    assert_eq!(
        norito::schema::identity::frame_hash::<PreReleaseSpendOp>(),
        norito::schema::identity::frame_hash::<SpendOp>(),
        "malformed fixture must use the production frame identity"
    );
    assert_eq!(
        norito::core::Header::read(shortened.as_slice())
            .expect("read malformed fixture header")
            .schema,
        norito::schema::identity::frame_hash::<SpendOp>(),
        "malformed fixture header must reach the production decoder"
    );
    assert!(
        decode_from_bytes::<SpendOp>(&shortened).is_err(),
        "the V1 spend operation must require its exact asset definition"
    );
}
#[test]
fn axt_v1_json_requires_nullable_slots_and_rejects_unknown_fields() {
    let dsid = DataSpaceId::new(20);
    let envelope = AxtProofEnvelope {
        dsid,
        manifest_root: [0xA6; 32],
        da_commitment: None,
        proof: vec![0xC4],
        fastpq_binding: Some(sample_fastpq_binding(dsid)),
        committed_amount: None,
        amount_commitment: None,
    };
    for field in [
        "da_commitment",
        "proof",
        "fastpq_binding",
        "committed_amount",
        "amount_commitment",
    ] {
        let mut value = norito::json::to_value(&envelope).expect("serialize AXT proof envelope");
        assert!(
            value
                .as_object_mut()
                .expect("AXT proof envelope JSON object")
                .remove(field)
                .is_some(),
            "fixture must contain field {field}"
        );
        assert!(
            norito::json::from_value::<AxtProofEnvelope>(value).is_err(),
            "the V1 proof envelope must require {field}"
        );
    }
    let mut unknown = norito::json::to_value(&envelope).expect("serialize AXT proof envelope");
    unknown
        .as_object_mut()
        .expect("AXT proof envelope JSON object")
        .insert("pre_release_field".to_owned(), norito::json::Value::Null);
    assert!(
        norito::json::from_value::<AxtProofEnvelope>(unknown).is_err(),
        "the V1 proof envelope must reject unknown fields"
    );

    let binding = envelope
        .fastpq_binding
        .as_ref()
        .expect("fixture has FASTPQ binding");
    let mut missing_effect = norito::json::to_value(binding).expect("serialize FASTPQ binding");
    missing_effect
        .as_object_mut()
        .expect("FASTPQ binding JSON object")
        .remove("effect_binding");
    assert!(
        norito::json::from_value::<AxtFastpqBinding>(missing_effect).is_err(),
        "the V1 FASTPQ binding must require its nullable effect slot"
    );

    let proof = ProofBlob {
        payload: vec![0xC5],
        expiry_slot: None,
    };
    let mut missing_expiry = norito::json::to_value(&proof).expect("serialize proof blob");
    missing_expiry
        .as_object_mut()
        .expect("proof blob JSON object")
        .remove("expiry_slot");
    assert!(
        norito::json::from_value::<ProofBlob>(missing_expiry).is_err(),
        "the V1 proof blob must require its nullable expiry slot"
    );
}
#[test]
fn axt_v1_json_requires_handle_and_envelope_collections() {
    let handle = sample_asset_handle();
    let mut missing_asset = norito::json::to_value(&handle).expect("serialize asset handle");
    missing_asset
        .as_object_mut()
        .expect("asset handle JSON object")
        .remove("asset_definition_id");
    assert!(
        norito::json::from_value::<AssetHandle>(missing_asset).is_err(),
        "the V1 asset handle must require its issuer-signed asset definition"
    );
    let mut missing_skew = norito::json::to_value(&handle).expect("serialize asset handle");
    missing_skew
        .as_object_mut()
        .expect("asset handle JSON object")
        .remove("max_clock_skew_ms");
    assert!(
        norito::json::from_value::<AssetHandle>(missing_skew).is_err(),
        "the V1 asset handle must require its nullable clock-skew slot"
    );

    let dsid = DataSpaceId::new(21);
    let record = AxtEnvelopeRecord {
        binding: AxtBinding::new([0xD1; 32]),
        lane: LaneId::new(2),
        descriptor: sample_descriptor(dsid),
        touches: Vec::new(),
        proofs: Vec::new(),
        handles: Vec::new(),
        commit_height: 1,
    };
    for field in ["touches", "proofs", "handles"] {
        let mut value = norito::json::to_value(&record).expect("serialize AXT envelope");
        value
            .as_object_mut()
            .expect("AXT envelope JSON object")
            .remove(field);
        assert!(
            norito::json::from_value::<AxtEnvelopeRecord>(value).is_err(),
            "the V1 AXT envelope must require {field}"
        );
    }
}
#[test]
#[expect(
    clippy::too_many_lines,
    reason = "the V1 JSON audit checks every nested mandatory nullable slot in one schema contract"
)]
fn axt_v1_json_requires_every_nested_nullable_slot() {
    macro_rules! assert_required_json_fields {
            ($value:expr, $ty:ty, [$($field:literal),+ $(,)?]) => {{
                let canonical = norito::json::to_value(&$value)
                    .expect("serialize canonical AXT JSON value");
                $(
                    let mut missing = canonical.clone();
                    assert!(
                        missing
                            .as_object_mut()
                            .expect("canonical AXT JSON object")
                            .remove($field)
                            .is_some(),
                        "fixture must contain field {}",
                        $field,
                    );
                    assert!(
                        norito::json::from_value::<$ty>(missing).is_err(),
                        "V1 AXT JSON must require field {}",
                        $field,
                    );
                )+
            }};
        }

    let effect = AxtEffectBinding {
        destination_domain: None,
        destination_account_id: None,
        vault_account_id: None,
        issuance_account_id: None,
        source_asset_definition_id: None,
        destination_asset_definition_id: None,
        source_amount_i64: None,
        destination_amount_i64: None,
    };
    assert_required_json_fields!(
        effect,
        AxtEffectBinding,
        [
            "destination_domain",
            "destination_account_id",
            "vault_account_id",
            "issuance_account_id",
            "source_asset_definition_id",
            "destination_asset_definition_id",
            "source_amount_i64",
            "destination_amount_i64",
        ]
    );

    let draft = sample_asset_handle_draft();
    assert_required_json_fields!(draft.subject, HandleSubject, ["origin_dsid"]);
    assert_required_json_fields!(draft.budget, HandleBudget, ["per_use"]);
    assert_required_json_fields!(
        draft,
        AssetHandleDraft,
        ["asset_definition_id", "max_clock_skew_ms"]
    );
    let context = issuer_context(test_network_id(b"json-v1-network"), DataSpaceId::new(7));
    assert_required_json_fields!(
        context,
        AxtHandleIssuerContextV1,
        ["asset_definition_incarnation"]
    );
    assert_required_json_fields!(
        draft.issuer_payload_v1(context),
        AssetHandleIssuerPayloadV1,
        ["asset_definition_id", "max_clock_skew_ms"]
    );
    let budget_key = AxtHandleBudgetKey::from_handle(&sample_asset_handle());
    assert_required_json_fields!(
        budget_key,
        AxtHandleBudgetKey,
        [
            "issuer_context",
            "asset_definition_id",
            "scope",
            "subject",
            "budget",
            "active_handle_era",
            "group_binding",
            "target_lane",
            "axt_binding",
            "manifest_view_root",
            "expiry_slot",
            "max_clock_skew_ms",
        ]
    );
    assert_required_json_fields!(
        AxtHandleBudgetRecord::empty(),
        AxtHandleBudgetRecord,
        ["consumed", "retain_until_slot"]
    );
    assert_required_json_fields!(
        AxtHandleCounterRecord::initial(1),
        AxtHandleCounterRecord,
        ["next", "authorization_generation"]
    );

    let op = SpendOp {
        asset_definition_id: test_asset_definition_id(),
        kind: "transfer".to_owned(),
        from: draft.subject.account.clone(),
        to: draft.subject.account.clone(),
        amount: None,
    };
    assert_required_json_fields!(op, SpendOp, ["asset_definition_id", "amount"]);
    assert_required_json_fields!(
        AxtHandleReplayKey::from_handle(DataSpaceId::new(7), &sample_asset_handle()),
        AxtHandleReplayKey,
        ["asset_definition_incarnation"]
    );
    assert_required_json_fields!(
        AxtReplayRecord {
            dataspace: DataSpaceId::new(7),
            budget_key: AxtHandleBudgetKey::from_handle(&sample_asset_handle()),
            used_slot: 1,
            retain_until_slot: 1,
        },
        AxtReplayRecord,
        ["budget_key"]
    );
    let fragment = AxtHandleFragment {
        handle: sample_asset_handle(),
        intent: RemoteSpendIntent {
            asset_dsid: DataSpaceId::new(7),
            op,
        },
        proof: None,
        amount: None,
        amount_commitment: None,
    };
    assert_required_json_fields!(
        fragment,
        AxtHandleFragment,
        ["proof", "amount", "amount_commitment"]
    );

    let reject = AxtRejectContext {
        reason: AxtRejectReason::PolicyDenied,
        dataspace: None,
        lane: None,
        snapshot_version: None,
        detail: "policy rejected".to_owned(),
        active_handle_era: None,
        next_handle_counter: None,
    };
    assert_required_json_fields!(
        reject,
        AxtRejectContext,
        [
            "dataspace",
            "lane",
            "snapshot_version",
            "active_handle_era",
            "next_handle_counter",
        ]
    );
}
fn descriptor_with_paths(read: &[&str], write: &[&str]) -> AxtDescriptor {
    let dsid = DataSpaceId::new(1);
    AxtDescriptor {
        dsids: vec![dsid],
        touches: vec![AxtTouchSpec {
            dsid,
            read: read.iter().map(|path| (*path).to_owned()).collect(),
            write: write.iter().map(|path| (*path).to_owned()).collect(),
        }],
    }
}
#[test]
fn touch_manifest_constructor_canonicalizes_paths() {
    let manifest = TouchManifest::from_read_write(
        [" zebra ", "", "alpha", "alpha", " \t "],
        [" zeta", "\n", "beta ", "beta", "alpha"],
    );
    assert_eq!(manifest.read, vec!["alpha".to_owned(), "zebra".to_owned()]);
    assert_eq!(
        manifest.write,
        vec!["alpha".to_owned(), "beta".to_owned(), "zeta".to_owned()]
    );
}
#[test]
fn descriptor_validation_rejects_duplicates_and_missing() {
    let empty = AxtDescriptor {
        dsids: Vec::new(),
        touches: Vec::new(),
    };
    assert!(matches!(
        validate_descriptor(&empty),
        Err(AxtValidationError::EmptyDataspaceList)
    ));
    let dup_ds = AxtDescriptor {
        dsids: vec![DataSpaceId::new(1), DataSpaceId::new(1)],
        touches: Vec::new(),
    };
    assert!(matches!(
        validate_descriptor(&dup_ds),
        Err(AxtValidationError::DuplicateDataspaceId(_))
    ));
    let undeclared_touch = AxtDescriptor {
        dsids: vec![DataSpaceId::new(2)],
        touches: vec![AxtTouchSpec {
            dsid: DataSpaceId::new(99),
            read: Vec::new(),
            write: Vec::new(),
        }],
    };
    assert!(matches!(
        validate_descriptor(&undeclared_touch),
        Err(AxtValidationError::TouchUndeclaredDataspace(_))
    ));
    let dup_touch = AxtDescriptor {
        dsids: vec![DataSpaceId::new(3)],
        touches: vec![
            AxtTouchSpec {
                dsid: DataSpaceId::new(3),
                read: Vec::new(),
                write: Vec::new(),
            },
            AxtTouchSpec {
                dsid: DataSpaceId::new(3),
                read: Vec::new(),
                write: Vec::new(),
            },
        ],
    };
    assert!(matches!(
        validate_descriptor(&dup_touch),
        Err(AxtValidationError::DuplicateTouch(_))
    ));
}
#[test]
fn descriptor_validation_rejects_noncanonical_entry_order() {
    let first = DataSpaceId::new(2);
    let second = DataSpaceId::new(1);
    let unsorted_dsids = AxtDescriptor {
        dsids: vec![first, second],
        touches: Vec::new(),
    };
    assert_eq!(
        validate_descriptor(&unsorted_dsids),
        Err(AxtValidationError::DataspaceIdsNotStrictlyOrdered {
            previous: first,
            current: second,
        })
    );
    let unsorted_touches = AxtDescriptor {
        dsids: vec![second, first],
        touches: vec![
            AxtTouchSpec {
                dsid: first,
                read: Vec::new(),
                write: Vec::new(),
            },
            AxtTouchSpec {
                dsid: second,
                read: Vec::new(),
                write: Vec::new(),
            },
        ],
    };
    assert_eq!(
        validate_descriptor(&unsorted_touches),
        Err(AxtValidationError::TouchesNotStrictlyOrdered {
            previous: first,
            current: second,
        })
    );
}
#[test]
fn descriptor_validation_rejects_noncanonical_read_paths() {
    assert_eq!(
        validate_descriptor(&descriptor_with_paths(&[" \t "], &[])),
        Err(AxtValidationError::EmptyReadPath {
            dsid: DataSpaceId::new(1),
            index: 0,
        })
    );
    assert_eq!(
        validate_descriptor(&descriptor_with_paths(&[" orders "], &[])),
        Err(AxtValidationError::UntrimmedReadPath {
            dsid: DataSpaceId::new(1),
            index: 0,
        })
    );
    assert_eq!(
        validate_descriptor(&descriptor_with_paths(&["orders", "orders"], &[])),
        Err(AxtValidationError::DuplicateReadPath {
            dsid: DataSpaceId::new(1),
            first_index: 0,
            duplicate_index: 1,
        })
    );
    assert_eq!(
        validate_descriptor(&descriptor_with_paths(&["zebra", "alpha"], &[])),
        Err(AxtValidationError::ReadPathsNotStrictlyOrdered {
            dsid: DataSpaceId::new(1),
            previous_index: 0,
            current_index: 1,
        })
    );
}
#[test]
fn descriptor_validation_rejects_noncanonical_write_paths() {
    assert_eq!(
        validate_descriptor(&descriptor_with_paths(&[], &["\n"])),
        Err(AxtValidationError::EmptyWritePath {
            dsid: DataSpaceId::new(1),
            index: 0,
        })
    );
    assert_eq!(
        validate_descriptor(&descriptor_with_paths(&[], &[" ledger "])),
        Err(AxtValidationError::UntrimmedWritePath {
            dsid: DataSpaceId::new(1),
            index: 0,
        })
    );
    assert_eq!(
        validate_descriptor(&descriptor_with_paths(&[], &["ledger", "ledger"])),
        Err(AxtValidationError::DuplicateWritePath {
            dsid: DataSpaceId::new(1),
            first_index: 0,
            duplicate_index: 1,
        })
    );
    assert_eq!(
        validate_descriptor(&descriptor_with_paths(&[], &["zebra", "alpha"])),
        Err(AxtValidationError::WritePathsNotStrictlyOrdered {
            dsid: DataSpaceId::new(1),
            previous_index: 0,
            current_index: 1,
        })
    );
}
#[test]
fn replay_record_zeroed_slots_are_expired() {
    let record = AxtReplayRecord {
        dataspace: DataSpaceId::new(1),
        budget_key: AxtHandleBudgetKey::from_handle(&sample_asset_handle()),
        used_slot: 0,
        retain_until_slot: 0,
    };
    assert!(record.is_expired(0, 1));
    assert!(record.is_expired(5, 10));
}
#[test]
fn replay_record_expires_strictly_after_effective_deadline() {
    let record = AxtReplayRecord {
        dataspace: DataSpaceId::new(1),
        budget_key: AxtHandleBudgetKey::from_handle(&sample_asset_handle()),
        used_slot: 10,
        retain_until_slot: 20,
    };
    assert!(!record.is_expired(19, 5));
    assert!(
        !record.is_expired(20, 5),
        "the handle remains valid through its inclusive expiry slot"
    );
    assert!(record.is_expired(21, 5));

    assert!(
        !record.is_expired(25, 15),
        "the configured retention window is also inclusive"
    );
    assert!(record.is_expired(26, 15));
}
#[test]
#[expect(
    clippy::too_many_lines,
    reason = "the replay audit keeps authoritative-key, canonical-storage, expiry, and conflict checks together"
)]
fn replay_record_validation_uses_authoritative_key_and_canonical_storage_key() {
    let key = AxtHandleReplayKey::from_parts(
        DataSpaceId::new(7),
        test_asset_incarnation(b"persisted-replay-key"),
        [0xA5; 32],
        3,
        4,
        LaneId::new(2),
    );
    let record = AxtReplayRecord {
        dataspace: DataSpaceId::new(7),
        budget_key: budget_key_for_replay_key(&key),
        used_slot: 10,
        retain_until_slot: 10,
    };
    assert_eq!(record.validate_for_key(&key), Ok(()));

    let mut zero_era = key;
    zero_era.handle_era = 0;
    assert_eq!(
        record.validate_for_key(&zero_era),
        Err(AxtReplayRecordValidationError::InvalidReplayKey(
            AxtHandleReplayKeyValidationError::ZeroHandleEra
        ))
    );
    let mut zero_sub_nonce = key;
    zero_sub_nonce.sub_nonce = 0;
    assert_eq!(
        record.validate_for_key(&zero_sub_nonce),
        Err(AxtReplayRecordValidationError::InvalidReplayKey(
            AxtHandleReplayKeyValidationError::ZeroSubNonce
        ))
    );

    let mut invalid = record.clone();
    invalid.dataspace = DataSpaceId::new(8);
    assert_eq!(
        invalid.validate_for_key(&key),
        Err(AxtReplayRecordValidationError::DataspaceMismatch)
    );
    let mut invalid = record.clone();
    invalid.budget_key.issuer_context.asset_dsid = DataSpaceId::new(8);
    assert_eq!(
        invalid.validate_for_key(&key),
        Err(AxtReplayRecordValidationError::DataspaceMismatch)
    );
    let mut invalid = record.clone();
    invalid
        .budget_key
        .issuer_context
        .asset_definition_incarnation = AxtAssetIncarnationV1(Hash::prehashed([0; Hash::LENGTH]));
    assert_eq!(
        invalid.validate_for_key(&key),
        Err(AxtReplayRecordValidationError::InvalidBudgetKey(
            AxtAssetIncarnationValidationError::Zero
        ))
    );
    let mut different_incarnation = key;
    different_incarnation.asset_definition_incarnation =
        test_asset_incarnation(b"different-replay-incarnation");
    let mut invalid = record.clone();
    invalid.budget_key = budget_key_for_replay_key(&different_incarnation);
    assert_eq!(
        invalid.validate_for_key(&key),
        Err(AxtReplayRecordValidationError::BudgetIncarnationMismatch)
    );
    let mut invalid = record.clone();
    invalid.budget_key.active_handle_era = key.handle_era + 1;
    assert_eq!(
        invalid.validate_for_key(&key),
        Err(AxtReplayRecordValidationError::BudgetGenerationMismatch)
    );
    let mut invalid = record.clone();
    invalid.budget_key.target_lane = LaneId::new(3);
    assert_eq!(
        invalid.validate_for_key(&key),
        Err(AxtReplayRecordValidationError::BudgetLaneMismatch)
    );
    let mut different_binding = key;
    different_binding.binding = AxtBinding::new([0xD6; 32]);
    let mut invalid = record.clone();
    invalid.budget_key = budget_key_for_replay_key(&different_binding);
    assert_eq!(
        invalid.validate_for_key(&key),
        Err(AxtReplayRecordValidationError::BudgetBindingMismatch)
    );
    let invalid = AxtReplayRecord {
        dataspace: DataSpaceId::new(7),
        budget_key: record.budget_key.clone(),
        used_slot: 0,
        retain_until_slot: 0,
    };
    assert_eq!(
        invalid.validate_for_key(&key),
        Err(AxtReplayRecordValidationError::ZeroedSlots)
    );
    let invalid = AxtReplayRecord {
        dataspace: DataSpaceId::new(7),
        budget_key: record.budget_key.clone(),
        used_slot: 11,
        retain_until_slot: 10,
    };
    assert_eq!(
        invalid.validate_for_key(&key),
        Err(AxtReplayRecordValidationError::RetentionBeforeUse)
    );

    {
        let mut encoded = String::new();
        key.encode_json_key(&mut encoded);
        let mut parser = norito::json::Parser::new(&encoded);
        let raw_key = parser.parse_string().expect("parse JSON replay key");
        assert_eq!(
            AxtHandleReplayKey::decode_json_key(&raw_key)
                .expect("decode canonical JSON replay key"),
            key
        );
        assert!(AxtHandleReplayKey::decode_json_key(&(raw_key + " ")).is_err());

        let mut missing_incarnation = norito::json::to_value(&key).expect("encode replay key JSON");
        missing_incarnation
            .as_object_mut()
            .expect("replay key JSON object")
            .remove("asset_definition_incarnation");
        assert!(
            norito::json::from_value::<AxtHandleReplayKey>(missing_incarnation).is_err(),
            "the first-release replay key must require its asset incarnation"
        );

        let mut invalid_key_json = norito::json::to_value(&key).expect("encode replay key JSON");
        let mut logical_zero = norito::json::to_value(&key.asset_definition_incarnation)
            .expect("encode incarnation JSON");
        logical_zero
            .as_array_mut()
            .expect("transparent incarnation JSON tuple")[0] =
            norito::json::to_value(&Hash::prehashed([0; Hash::LENGTH]))
                .expect("encode logical-zero hash");
        invalid_key_json
            .as_object_mut()
            .expect("replay key JSON object")
            .insert("asset_definition_incarnation".to_owned(), logical_zero);
        let invalid_key_json =
            norito::json::to_string(&invalid_key_json).expect("serialize invalid replay key");
        let invalid_key: AxtHandleReplayKey = norito::json::from_str(&invalid_key_json)
            .expect("logical-zero hash is syntactically decodable");
        assert_eq!(
            invalid_key.validate(),
            Err(AxtHandleReplayKeyValidationError::InvalidAssetIncarnation(
                AxtAssetIncarnationValidationError::Zero
            ))
        );
        assert_eq!(
            record.validate_for_key(&invalid_key),
            Err(AxtReplayRecordValidationError::InvalidReplayKey(
                AxtHandleReplayKeyValidationError::InvalidAssetIncarnation(
                    AxtAssetIncarnationValidationError::Zero
                )
            ))
        );
        assert!(AxtHandleReplayKey::decode_json_key(&invalid_key_json).is_err());
    }
}
#[test]
fn descriptor_validation_accepts_valid_descriptor() {
    let descriptor = sample_descriptor(DataSpaceId::new(7));
    assert_eq!(validate_descriptor(&descriptor), Ok(()));
}
#[test]
fn descriptor_binding_hashes_bare_norito_payload() {
    let descriptor = sample_descriptor(DataSpaceId::new(9));
    let mut expected_preimage = b"iroha:axt:desc:v1\0".to_vec();
    expected_preimage.extend_from_slice(&encode_adaptive(&descriptor));
    let binding = compute_descriptor_binding(&descriptor).expect("binding");
    assert_eq!(binding, poseidon_hash_bytes(&expected_preimage));
}
#[test]
fn axt_reject_reason_roundtrips_label() {
    assert_eq!(
        AxtRejectReason::from_label(AxtRejectReason::HandleEra.label()),
        Some(AxtRejectReason::HandleEra)
    );
    assert_eq!(AxtRejectReason::from_label("unknown"), None);
}
#[test]
#[expect(
    clippy::too_many_lines,
    reason = "one canonical envelope fixture verifies the full nested wire shape and required commit height"
)]
fn envelope_roundtrips_through_norito() {
    #[derive(Encode, norito::NoritoSchema)]
    #[norito_schema(
        name = "test::iroha_data_model::nexus::axt::EnvelopeWithoutCommitHeight",
        frame = "iroha_data_model::nexus::axt::AxtEnvelopeRecord"
    )]
    struct EnvelopeWithoutCommitHeight {
        binding: AxtBinding,
        lane: LaneId,
        descriptor: AxtDescriptor,
        touches: Vec<AxtTouchFragment>,
        proofs: Vec<AxtProofFragment>,
        handles: Vec<AxtHandleFragment>,
    }
    let dsid = DataSpaceId::new(11);
    let descriptor = sample_descriptor(dsid);
    let binding = AxtBinding::new([0xAB; 32]);
    let alice_account = crate::account::AccountId::new(
        "ed0120EDF6D7B52C7032D03AEC696F2068BD53101528F3C7B6081BFF05A1662D7FC245"
            .parse()
            .expect("public key"),
    )
    .to_string();
    let merchant_account = crate::account::AccountId::new(
        "ed0120A98BAFB0663CE08D75EBD506FEC38A84E576A7C9B0897693ED4B04FD9EF2D18D"
            .parse()
            .expect("public key"),
    )
    .to_string();
    let envelope = AxtEnvelopeRecord {
        binding,
        lane: LaneId::new(1),
        descriptor: descriptor.clone(),
        touches: vec![AxtTouchFragment {
            dsid,
            manifest: TouchManifest {
                read: vec!["orders/0".into()],
                write: vec!["ledger/0".into()],
            },
        }],
        proofs: vec![AxtProofFragment {
            dsid,
            proof: ProofBlob {
                payload: vec![0xA5, 0x5A],
                expiry_slot: None,
            },
        }],
        handles: vec![AxtHandleFragment {
            handle: AssetHandle {
                asset_definition_id: test_asset_definition_id(),
                scope: vec!["transfer".into()],
                subject: HandleSubject {
                    account: alice_account.clone(),
                    origin_dsid: Some(dsid),
                },
                budget: HandleBudget {
                    remaining: Quantity::from(500_u64),
                    per_use: Some(Quantity::from(300_u64)),
                },
                handle_era: 1,
                sub_nonce: 42,
                group_binding: GroupBinding {
                    composability_group_id: vec![0u8; 32],
                    epoch_id: 1,
                },
                target_lane: LaneId::new(0),
                axt_binding: binding,
                manifest_view_root: [1u8; 32],
                expiry_slot: 10,
                max_clock_skew_ms: Some(0),
                issuer_context: AxtHandleIssuerContextV1 {
                    network_id: test_network_id(b"envelope-roundtrip-network"),
                    asset_dsid: dsid,
                    asset_definition_incarnation: AxtAssetIncarnationV1::derive(
                        &test_network_id(b"envelope-roundtrip-network"),
                        &test_asset_definition_id(),
                        &HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
                            b"envelope-roundtrip-asset-registration-header",
                        )),
                        &Hash::new(b"envelope-roundtrip-asset-registration-execution"),
                        0,
                    ),
                    issuer: UniversalAccountId::from_hash(Hash::new(b"envelope-roundtrip-issuer")),
                    issuer_manifest_root: [1u8; 32],
                    code_root: Hash::new(b"envelope-roundtrip-code").into(),
                    abi_version: 1,
                    abi_hash: ivm_abi::syscalls::compute_abi_hash(ivm_abi::SyscallPolicy::AbiV1),
                },
                issuer_signature: iroha_crypto::Signature::from_bytes(&[1_u8; 64]),
            },
            intent: RemoteSpendIntent {
                asset_dsid: dsid,
                op: SpendOp {
                    asset_definition_id: test_asset_definition_id(),
                    kind: "transfer".into(),
                    from: alice_account,
                    to: merchant_account,
                    amount: Some(Quantity::from(200_u64)),
                },
            },
            proof: Some(ProofBlob {
                payload: vec![0xCC],
                expiry_slot: None,
            }),
            amount: Some(Quantity::from(200_u64)),
            amount_commitment: None,
        }],
        commit_height: 5,
    };
    let bytes = to_bytes(&envelope).expect("encode envelope");
    let decoded: AxtEnvelopeRecord = decode_from_bytes(&bytes).expect("decode envelope");
    assert_eq!(decoded, envelope);
    assert_eq!(decoded.binding.as_bytes(), &binding.into_array());
    assert_eq!(decoded.descriptor, descriptor);
    let missing_commit_height = EnvelopeWithoutCommitHeight {
        binding: envelope.binding,
        lane: envelope.lane,
        descriptor: envelope.descriptor.clone(),
        touches: envelope.touches.clone(),
        proofs: envelope.proofs.clone(),
        handles: envelope.handles.clone(),
    };
    let missing_commit_height_bytes =
        to_bytes(&missing_commit_height).expect("encode omitted-height fixture");
    assert_eq!(
        norito::schema::identity::frame_hash::<EnvelopeWithoutCommitHeight>(),
        norito::schema::identity::frame_hash::<AxtEnvelopeRecord>(),
        "malformed fixture must use the production frame identity"
    );
    assert_eq!(
        norito::core::Header::read(missing_commit_height_bytes.as_slice())
            .expect("read malformed fixture header")
            .schema,
        norito::schema::identity::frame_hash::<AxtEnvelopeRecord>(),
        "malformed fixture header must reach the production decoder"
    );
    assert!(
        decode_from_bytes::<AxtEnvelopeRecord>(&missing_commit_height_bytes).is_err(),
        "commit_height is a required V1 wire field"
    );
}
#[test]
#[expect(
    clippy::too_many_lines,
    reason = "one policy-snapshot matrix covers canonical order, required fields, duplicates, and version binding"
)]
fn policy_snapshot_validation_rejects_order_duplicates_and_stale_versions() {
    #[derive(Encode, norito::NoritoSchema)]
    #[norito_schema(
        name = "test::iroha_data_model::nexus::axt::SnapshotWithoutVersion",
        frame = "iroha_data_model::nexus::axt::AxtPolicySnapshot"
    )]
    struct SnapshotWithoutVersion {
        entries: Vec<AxtPolicyBinding>,
    }
    #[derive(Encode, norito::NoritoSchema)]
    #[norito_schema(
        name = "test::iroha_data_model::nexus::axt::SnapshotWithoutEntries",
        frame = "iroha_data_model::nexus::axt::AxtPolicySnapshot"
    )]
    struct SnapshotWithoutEntries {
        version: u64,
    }
    let policy = AxtPolicyEntry {
        manifest_root: [0x42; 32],
        target_lane: LaneId::new(1),
        active_handle_era: 1,
        next_handle_counter: 1,
        current_slot: 1,
    };
    let first = AxtPolicyBinding {
        dsid: DataSpaceId::new(1),
        policy,
    };
    let second = AxtPolicyBinding {
        dsid: DataSpaceId::new(2),
        policy,
    };
    let entries = vec![first, second];
    let canonical = AxtPolicySnapshot {
        version: AxtPolicySnapshot::compute_version(&entries),
        entries,
    };
    assert_eq!(canonical.validate(), Ok(()));
    assert_eq!(AxtPolicySnapshot::default().validate(), Ok(()));
    let missing_version_bytes = to_bytes(&SnapshotWithoutVersion {
        entries: canonical.entries.clone(),
    })
    .expect("encode missing-version snapshot");
    assert_eq!(
        norito::schema::identity::frame_hash::<SnapshotWithoutVersion>(),
        norito::schema::identity::frame_hash::<AxtPolicySnapshot>(),
        "malformed fixture must use the production frame identity"
    );
    assert_eq!(
        norito::core::Header::read(missing_version_bytes.as_slice())
            .expect("read malformed fixture header")
            .schema,
        norito::schema::identity::frame_hash::<AxtPolicySnapshot>(),
        "malformed fixture header must reach the production decoder"
    );
    assert!(
        decode_from_bytes::<AxtPolicySnapshot>(&missing_version_bytes).is_err(),
        "snapshot version is a required V1 wire field"
    );
    let missing_entries_bytes = to_bytes(&SnapshotWithoutEntries {
        version: canonical.version,
    })
    .expect("encode missing-entries snapshot");
    assert_eq!(
        norito::schema::identity::frame_hash::<SnapshotWithoutEntries>(),
        norito::schema::identity::frame_hash::<AxtPolicySnapshot>(),
        "malformed fixture must use the production frame identity"
    );
    assert_eq!(
        norito::core::Header::read(missing_entries_bytes.as_slice())
            .expect("read malformed fixture header")
            .schema,
        norito::schema::identity::frame_hash::<AxtPolicySnapshot>(),
        "malformed fixture header must reach the production decoder"
    );
    assert!(
        decode_from_bytes::<AxtPolicySnapshot>(&missing_entries_bytes).is_err(),
        "snapshot entries are a required V1 wire field"
    );
    let duplicate_entries = vec![first, first];
    let duplicate = AxtPolicySnapshot {
        version: AxtPolicySnapshot::compute_version(&duplicate_entries),
        entries: duplicate_entries,
    };
    assert_eq!(
        duplicate.validate(),
        Err(AxtPolicySnapshotValidationError::DuplicateDataspaceId(
            first.dsid
        ))
    );
    let reversed_entries = vec![second, first];
    let reversed = AxtPolicySnapshot {
        version: AxtPolicySnapshot::compute_version(&reversed_entries),
        entries: reversed_entries,
    };
    assert_ne!(
        canonical.version, reversed.version,
        "snapshot versions must bind the exact entry order"
    );
    assert_eq!(
        reversed.validate(),
        Err(
            AxtPolicySnapshotValidationError::EntriesNotStrictlyOrdered {
                previous: second.dsid,
                current: first.dsid,
            }
        )
    );
    assert!(matches!(
        reversed.clone().with_computed_version(),
        Err(AxtPolicySnapshotValidationError::EntriesNotStrictlyOrdered { .. })
    ));
    let zero_version = AxtPolicySnapshot {
        version: 0,
        entries: canonical.entries.clone(),
    };
    assert_eq!(
        zero_version.validate(),
        Err(AxtPolicySnapshotValidationError::VersionMismatch {
            expected: canonical.version,
            actual: 0,
        })
    );
    let stale = AxtPolicySnapshot {
        version: canonical.version.wrapping_add(1),
        entries: canonical.entries.clone(),
    };
    assert_eq!(
        stale.validate(),
        Err(AxtPolicySnapshotValidationError::VersionMismatch {
            expected: canonical.version,
            actual: stale.version,
        })
    );
}
#[test]
fn proof_envelope_shape_matches_manifest_accepts_envelope_and_rejects_raw_root() {
    let dsid = DataSpaceId::new(17);
    let manifest_root = [0xA5; 32];
    let envelope = AxtProofEnvelope {
        dsid,
        manifest_root,
        da_commitment: None,
        proof: vec![0xCC],
        fastpq_binding: Some(sample_fastpq_binding(dsid)),
        committed_amount: None,
        amount_commitment: None,
    };
    let encoded = norito::to_bytes(&envelope).expect("encode envelope");
    let proof = ProofBlob {
        payload: encoded,
        expiry_slot: None,
    };
    assert!(proof_envelope_shape_matches_manifest(
        &proof,
        dsid,
        manifest_root
    ));
    let missing_binding = AxtProofEnvelope {
        dsid,
        manifest_root,
        da_commitment: None,
        proof: vec![0xCC],
        fastpq_binding: None,
        committed_amount: None,
        amount_commitment: None,
    };
    let missing_binding_proof = ProofBlob {
        payload: norito::to_bytes(&missing_binding).expect("encode envelope"),
        expiry_slot: None,
    };
    assert!(!proof_envelope_shape_matches_manifest(
        &missing_binding_proof,
        dsid,
        manifest_root
    ));
    let raw_proof = ProofBlob {
        payload: manifest_root.to_vec(),
        expiry_slot: Some(5),
    };
    assert!(!proof_envelope_shape_matches_manifest(
        &raw_proof,
        dsid,
        manifest_root
    ));
    let mut oversized_envelope = envelope;
    oversized_envelope.proof = vec![0; MAX_AXT_PROOF_BLOB_PAYLOAD_BYTES];
    let oversized_payload =
        norito::to_bytes(&oversized_envelope).expect("encode oversized canonical envelope");
    assert!(oversized_payload.len() > MAX_AXT_PROOF_BLOB_PAYLOAD_BYTES);
    let oversized = ProofBlob {
        payload: oversized_payload,
        expiry_slot: None,
    };
    assert!(!proof_envelope_shape_matches_manifest(
        &oversized,
        dsid,
        manifest_root
    ));
}
#[test]
fn fastpq_binding_shape_requires_strictly_increasing_target_dsids() {
    let mut binding = sample_fastpq_binding(DataSpaceId::new(17));
    binding.target_dsids = vec![1, 2, 3];
    assert!(fastpq_binding_shape_is_concrete(&binding));
    binding.target_dsids = vec![1, 1, 2];
    assert!(!fastpq_binding_shape_is_concrete(&binding));
    binding.target_dsids = vec![3, 1, 2];
    assert!(
        !fastpq_binding_shape_is_concrete(&binding),
        "unique but non-canonical target order must fail closed"
    );
}
#[test]
fn fastpq_binding_shape_requires_canonical_remote_spend_commitment_set() {
    let mut binding = sample_fastpq_binding(DataSpaceId::new(18));
    binding.remote_spend_intent_commitments = vec![[0x11; 32], [0x22; 32]];
    assert!(fastpq_binding_shape_is_concrete(&binding));
    binding.remote_spend_intent_commitments = vec![[0x11; 32], [0x11; 32]];
    assert!(!fastpq_binding_shape_is_concrete(&binding));
    binding.remote_spend_intent_commitments = vec![[0x22; 32], [0x11; 32]];
    assert!(!fastpq_binding_shape_is_concrete(&binding));

    binding.remote_spend_intent_commitments = (0_u64
        ..=u64::try_from(MAX_REMOTE_SPEND_INTENT_COMMITMENTS_V1)
            .expect("V1 commitment limit fits u64"))
        .map(|index| {
            let mut commitment = [0_u8; 32];
            commitment[24..].copy_from_slice(&index.to_be_bytes());
            commitment
        })
        .collect();
    assert!(
        !fastpq_binding_shape_is_concrete(&binding),
        "an ordered but oversized commitment set must fail closed"
    );
}
#[test]
fn proof_envelope_shape_matches_manifest_rejects_alternate_layout_and_restores_flags() {
    let dsid = DataSpaceId::new(21);
    let manifest_root = [0xD2; 32];
    let envelope = AxtProofEnvelope {
        dsid,
        manifest_root,
        da_commitment: None,
        proof: vec![0xCC],
        fastpq_binding: Some(sample_fastpq_binding(dsid)),
        committed_amount: None,
        amount_commitment: None,
    };
    let default_flags = norito::default_encode_flags();
    let alternate_flags = default_flags & !norito::core::header_flags::COMPACT_LEN;
    assert_ne!(alternate_flags, default_flags);
    let prior_flags = norito::core::effective_decode_flags();
    let canonical_payload = {
        let _guard = norito::core::DecodeFlagsGuard::enter(default_flags);
        norito::to_bytes(&envelope).expect("encode canonical envelope")
    };
    let alternate_payload = {
        let _guard = norito::core::DecodeFlagsGuard::enter(alternate_flags);
        norito::to_bytes(&envelope).expect("encode alternate-layout envelope")
    };
    assert_ne!(alternate_payload, canonical_payload);
    let canonical_proof = ProofBlob {
        payload: canonical_payload,
        expiry_slot: None,
    };
    let alternate_proof = ProofBlob {
        payload: alternate_payload,
        expiry_slot: None,
    };
    {
        let _caller_guard = norito::core::DecodeFlagsGuard::enter(alternate_flags);
        assert_eq!(
            norito::core::effective_decode_flags(),
            Some(alternate_flags)
        );
        assert!(proof_envelope_shape_matches_manifest(
            &canonical_proof,
            dsid,
            manifest_root
        ));
        assert_eq!(
            norito::core::effective_decode_flags(),
            Some(alternate_flags)
        );
        assert!(!proof_envelope_shape_matches_manifest(
            &alternate_proof,
            dsid,
            manifest_root
        ));
        assert_eq!(
            norito::core::effective_decode_flags(),
            Some(alternate_flags)
        );
    }
    assert_eq!(norito::core::effective_decode_flags(), prior_flags);
}
#[test]
fn proof_envelope_shape_matches_manifest_rejects_synthetic_binding_shape() {
    let dsid = DataSpaceId::new(20);
    let manifest_root = [0xC1; 32];
    let mut binding = sample_fastpq_binding(dsid);
    binding.claim_digest.clear();
    let envelope = AxtProofEnvelope {
        dsid,
        manifest_root,
        da_commitment: None,
        proof: vec![0xCC],
        fastpq_binding: Some(binding),
        committed_amount: None,
        amount_commitment: None,
    };
    let proof = ProofBlob {
        payload: norito::to_bytes(&envelope).expect("encode envelope"),
        expiry_slot: None,
    };
    assert!(!proof_envelope_shape_matches_manifest(
        &proof,
        dsid,
        manifest_root
    ));
    let mut binding = sample_fastpq_binding(dsid);
    binding.claim_type = "synthetic".to_string();
    let envelope = AxtProofEnvelope {
        dsid,
        manifest_root,
        da_commitment: None,
        proof: vec![0xCC],
        fastpq_binding: Some(binding),
        committed_amount: None,
        amount_commitment: None,
    };
    let proof = ProofBlob {
        payload: norito::to_bytes(&envelope).expect("encode envelope"),
        expiry_slot: None,
    };
    assert!(!proof_envelope_shape_matches_manifest(
        &proof,
        dsid,
        manifest_root
    ));
    let mut binding = sample_fastpq_binding(dsid);
    binding.target_dsids.clear();
    let envelope = AxtProofEnvelope {
        dsid,
        manifest_root,
        da_commitment: None,
        proof: vec![0xCC],
        fastpq_binding: Some(binding),
        committed_amount: None,
        amount_commitment: None,
    };
    let proof = ProofBlob {
        payload: norito::to_bytes(&envelope).expect("encode envelope"),
        expiry_slot: None,
    };
    assert!(!proof_envelope_shape_matches_manifest(
        &proof,
        dsid,
        manifest_root
    ));
}
#[test]
fn proof_envelope_shape_matches_manifest_rejects_mismatch() {
    let dsid = DataSpaceId::new(18);
    let other = DataSpaceId::new(19);
    let manifest_root = [0xB4; 32];
    let bad_root = [0xB5; 32];
    let envelope = AxtProofEnvelope {
        dsid: other,
        manifest_root: bad_root,
        da_commitment: None,
        proof: vec![0xCC],
        fastpq_binding: Some(sample_fastpq_binding(other)),
        committed_amount: None,
        amount_commitment: None,
    };
    let encoded = norito::to_bytes(&envelope).expect("encode envelope");
    let proof = ProofBlob {
        payload: encoded,
        expiry_slot: None,
    };
    assert!(!proof_envelope_shape_matches_manifest(
        &proof,
        dsid,
        manifest_root
    ));
    let raw_proof = ProofBlob {
        payload: bad_root.to_vec(),
        expiry_slot: Some(7),
    };
    assert!(!proof_envelope_shape_matches_manifest(
        &raw_proof,
        dsid,
        manifest_root
    ));
    let zero_root = [0u8; 32];
    let zero_proof = ProofBlob {
        payload: zero_root.to_vec(),
        expiry_slot: None,
    };
    assert!(!proof_envelope_shape_matches_manifest(
        &zero_proof,
        dsid,
        zero_root
    ));
}
