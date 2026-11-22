//! Core30 ordinary Receive evidence for the funded test catalog. Receiver and payer
//! independently read the canonical wallet envelope. This fixture installs no wallet keys.
use super::*;
use iroha_data_model::kagemusha::{
    KagemushaDevicePublicKeyV1, KagemushaDeviceSignatureV1, KagemushaWalletCreditedEvidenceV1,
    KagemushaWalletCreditedV1, KagemushaWalletEffectV1, KagemushaWalletEnvelopeV1,
    KagemushaWalletLifecycleV1, KagemushaWalletLineageSlotV1, KagemushaWalletMessageV1,
    KagemushaWalletOperationKindV1, KagemushaWalletPackageV1, KagemushaWalletReceiptSignerV1,
    KagemushaWalletReceiptV1, KagemushaWalletStateCommitmentV1, KagemushaWalletStatementV1,
    KagemushaWalletStepProofV1, kagemusha_wallet_proof_digest_v1,
};

fn id(words: &[Fp]) -> [u8; 32] {
    bootstrap_objects::id(words[0], words[1])
        .try_into()
        .unwrap()
}
fn integer(word: Fp) -> u128 {
    let raw = word.to_repr();
    assert_eq!(raw[16..], [0; 16]);
    u128::from_le_bytes(raw[..16].try_into().unwrap())
}

fn receive_statement(s: &[Fp; 26]) -> KagemushaWalletStatementV1 {
    let statement = KagemushaWalletStatementV1 {
        version: 1,
        scheme_id: id(&s[3..5]),
        relation_id: id(&s[1..3]),
        asset_digest: id(&s[5..7]),
        credential_digest: s[7].to_repr(),
        lifecycle: match integer(s[8]) {
            1 => KagemushaWalletLifecycleV1::Active,
            2 => KagemushaWalletLifecycleV1::Retiring,
            _ => panic!("fixture receiver lifecycle"),
        },
        sequence: integer(s[9]),
        next_load: integer(s[10]),
        enabled_controls: integer(s[11]).try_into().unwrap(),
        lineage_burned_total: integer(s[12]),
        lineage_pending_outgoing_root: s[13].to_repr(),
        predecessor: KagemushaWalletStateCommitmentV1 {
            value: s[14].to_repr(),
        },
        successor: KagemushaWalletStateCommitmentV1 {
            value: s[15].to_repr(),
        },
        effect: KagemushaWalletEffectV1::Receive {
            credit_id: s[17].to_repr(),
            payer_wallet_id: id(&s[18..20]),
            amount: integer(s[20]),
        },
    };
    assert_eq!(statement.field_items().unwrap(), s.map(|f| f.to_repr()));
    statement
}

/// A folded signer is not required for this evidence form. The receiver's quote
/// supplies the signer and the payer's locally verified Send supplies the expected effect.
#[derive(Clone)]
struct HeldCredit {
    signer: KagemushaWalletReceiptSignerV1,
    relation: [u8; 32],
    asset: [u8; 32],
    credit: [u8; 32],
    payer: [u8; 32],
    amount: u128,
    payment: [u8; 32],
}
impl HeldCredit {
    fn accepts(&self, bytes: &[u8], key: &crate::SigmaVerifier<Eq>) -> bool {
        let Ok(envelope) =
            KagemushaWalletEnvelopeV1::decode_canonical(bytes, &self.signer.scheme_id)
        else {
            return false;
        };
        let KagemushaWalletMessageV1::Credited { credited } = envelope.message else {
            return false;
        };
        let KagemushaWalletCreditedEvidenceV1::Receive { package } = credited.evidence else {
            return false;
        };
        let s = &package.statement;
        let expected = KagemushaWalletEffectV1::Receive {
            credit_id: self.credit,
            payer_wallet_id: self.payer,
            amount: self.amount,
        };
        if s.relation_id != self.relation
            || s.asset_digest != self.asset
            || s.effect != expected
            || package.receipt.payment_digest != self.payment
        {
            return false;
        }
        let Ok(digests) = package.verify_with(&self.signer) else {
            return false;
        };
        let Some(statement) = Option::<Fp>::from(Fp::from_repr(digests.statement)) else {
            return false;
        };
        key.verify(&StepPublic { statement }, &package.step_proof.bytes)
            .is_ok()
    }
}

fn encode(package: &KagemushaWalletPackageV1) -> Vec<u8> {
    let credited = KagemushaWalletCreditedV1::from_receive(package.clone()).unwrap();
    KagemushaWalletEnvelopeV1::new(KagemushaWalletMessageV1::Credited { credited })
        .to_canonical_bytes()
        .unwrap()
}

pub(super) fn verify_receiver(
    send: &send_objects::SendFixture,
    receiver: &crate::admin_sigma::BootstrapWitness,
    debit: &[Fp; 26],
    request: &bootstrap_objects::Signed,
    receiver_credential: &bootstrap_objects::Signed,
    payment_transcript: &[u8],
    root: Fp,
    expected_head: Fp,
) {
    let payment = p_bytes_native::<Fp>(u64::from_le_bytes(*b"kgwpay_1"), payment_transcript);
    let (receive, _) = receive_objects::monetary_step(
        send,
        &crate::admin_sigma::StateWitness::from(receiver),
        true,
    );
    let mut opened = receive.clone();
    opened.core30_prior_root = Some(root);
    let request_body = send.step.request_body();
    assert_eq!(request_body, receive.request_body());
    assert_eq!(
        request.bytes[..ObjectKind::Request.body_len()],
        send_objects::request_body_bytes(&request_body)
    );
    let statement = opened.statement(SigmaRelation::RECEIVE).unwrap();
    let accepted =
        crate::consumer::check_receive(&id(&debit[1..3]), &request_body, &statement).unwrap();
    let credit = opened.evaluate(SigmaRelation::RECEIVE);
    assert_eq!(accepted.relation, SigmaRelation::RECEIVE);
    assert_eq!(accepted.public, credit.public());
    assert!(credit.is_honest());
    assert_eq!(credit.statement[14], expected_head);
    let (key, sigma, ordinary) = prove_candidate_step(SigmaRelation::RECEIVE, receive, root);
    assert_eq!(sigma.public, credit.public());
    let st = receive_statement(&credit.statement);
    let step = KagemushaWalletStepProofV1 {
        bytes: sigma.bytes.clone(),
    };
    let proof = Fp::from_repr(
        kagemusha_wallet_proof_digest_v1(KagemushaWalletOperationKindV1::Receive, None, &step)
            .unwrap(),
    )
    .unwrap();
    let signer = KagemushaWalletReceiptSignerV1 {
        scheme_id: receiver_credential.bytes[2..34].try_into().unwrap(),
        wallet_id: receiver_credential.bytes[66..98].try_into().unwrap(),
        provider_contract: receiver_credential.bytes[195..227].try_into().unwrap(),
        payment_key: KagemushaDevicePublicKeyV1::from_sec1_bytes(
            &receiver_credential.bytes[130..195],
        )
        .unwrap(),
    };
    assert_eq!(request.bytes[130..162], signer.wallet_id);
    assert_eq!(
        request.bytes[210..242],
        receiver_credential.digest().to_repr()
    );
    assert_eq!(request.digest(), debit[23]);
    let held = HeldCredit {
        signer,
        relation: id(&debit[1..3]),
        asset: id(&debit[5..7]),
        credit: debit[17].to_repr(),
        payer: request.bytes[66..98].try_into().unwrap(),
        amount: integer(debit[21]),
        payment: payment.to_repr(),
    };
    let sign = |proof: Fp, digest: Fp, secret: u64, nonce: u64| {
        let raw = receipt(
            &credit.statement,
            &receiver.lineage[6..8],
            proof,
            digest,
            secret,
            nonce,
        );
        KagemushaWalletReceiptV1 {
            version: 1,
            operation_id: raw.bytes[114..146].try_into().unwrap(),
            capsule_commitment: raw.bytes[274..306].try_into().unwrap(),
            payment_digest: digest.to_repr(),
            signature: KagemushaDeviceSignatureV1::from_raw_bytes(&raw.bytes[338..]).unwrap(),
        }
    };
    let secret = bootstrap_objects::Identity::Receiver.secret();
    let mut package = KagemushaWalletPackageV1::new(
        st,
        KagemushaWalletLineageSlotV1::None,
        step,
        sign(proof, payment, secret, 89),
    );
    let bytes = encode(&package);
    let independent = crate::SigmaVerifier::from_bytes(
        SigmaRelation::RECEIVE,
        PinnedParams::<Eq>::derive(key.params().k()).unwrap(),
        key.descriptor_bytes(),
        key.vk_bytes(),
    )
    .unwrap();
    assert!(held.accepts(&bytes, &independent));
    if let Some(dir) = std::env::var_os("KAGEMUSHA_CORE30_UNFOLDED_OUTPUT") {
        let dir = std::path::PathBuf::from(dir);
        for (name, original) in [
            ("unfolded-credited.norito", bytes.as_slice()),
            ("payment-digest-transcript.bin", payment_transcript),
            ("signed-request.bin", &request.bytes),
            ("receiver-credential.bin", &receiver_credential.bytes),
            ("receive.sigma", &package.step_proof.bytes),
            ("receive.vk", key.vk_bytes()),
            ("receive.descriptor", key.descriptor_bytes()),
        ] {
            std::fs::write(dir.join(name), original).unwrap();
        }
    }
    for held in [
        HeldCredit {
            amount: held.amount + 1,
            ..held.clone()
        },
        HeldCredit {
            credit: (debit[17] + Fp::ONE).to_repr(),
            ..held.clone()
        },
        HeldCredit {
            payer: signer.wallet_id,
            ..held.clone()
        },
        HeldCredit {
            relation: held.asset,
            ..held.clone()
        },
        HeldCredit {
            asset: held.relation,
            ..held.clone()
        },
    ] {
        assert!(!held.accepts(&bytes, &independent));
    }
    package.receipt = sign(proof, payment + Fp::ONE, secret, 97);
    assert!(package.verify_with(&signer).is_ok());
    assert!(!held.accepts(&encode(&package), &independent));
    package.receipt = sign(
        proof,
        payment,
        bootstrap_objects::Identity::Payer.secret(),
        101,
    );
    assert!(!held.accepts(&encode(&package), &independent));
    *package.step_proof.bytes.last_mut().unwrap() ^= 1;
    let changed = Fp::from_repr(package.proof_digest().unwrap()).unwrap();
    assert_ne!(changed, proof);
    package.receipt = sign(changed, payment, secret, 103);
    assert!(package.verify_with(&signer).is_ok());
    assert!(!held.accepts(&encode(&package), &independent));
    assert!(!held.accepts(&bytes[..bytes.len() - 1], &independent));
    println!(
        "CORE30_UNFOLDED_CREDIT sigma_bytes={} envelope_bytes={} ipm1_uncompressed_bytes={} exact_receiver_signature=true wrong_payment_refused=true wrong_signer_refused=true changed_proof_refused=true",
        package.step_proof.bytes.len(),
        bytes.len(),
        bytes.len() + 84
    );
    // ArchiveReceive consumes exactly this unfolded Receive proof in its only incoming σ slot.
    let (incoming, vk, descriptor) = prove_incoming_q(
        SigmaRelation::RECEIVE,
        key,
        sigma,
        ordinary,
        6,
        Instant::now(),
    );
    if let Some(dir) = std::env::var_os("KAGEMUSHA_CORE30_UNFOLDED_OUTPUT") {
        let dir = std::path::PathBuf::from(dir);
        std::fs::write(dir.join("archive-receive-incoming.q"), incoming.bytes).unwrap();
        std::fs::write(dir.join("archive-receive-incoming.vk"), vk.to_bytes()).unwrap();
        std::fs::write(
            dir.join("archive-receive-incoming.descriptor"),
            descriptor.encoded(),
        )
        .unwrap();
        let instances: Vec<Vec<[u8; 32]>> = incoming
            .instances
            .iter()
            .map(|column| column.iter().map(PrimeField::to_repr).collect())
            .collect();
        std::fs::write(
            dir.join("archive-receive-incoming-instances.norito"),
            norito::encode_canonical(&instances).unwrap(),
        )
        .unwrap();
    }
}

/// Isolate the ordinary receiver package before the slower funded Ω fixture checks
/// that the payer actually owns the quoted predecessor and authenticates its Payment.
#[test]
#[ignore = "generates and checks the full ordinary Core30 Receive proof"]
fn ordinary_receive_package_binds_the_supplied_payment_digest() {
    let (payer, _, _) = bootstrap_objects::enrollment();
    let (load, _, _) = load_objects::funded(&payer);
    let signed = send_objects::from_load(&load.successor);
    let (receiver, _, credential) = receive_objects::receiver();
    let request = paired_request::verify_signed_original(
        ObjectKind::Request,
        &signed.objects[1],
        &credential.bytes[130..195],
    )
    .unwrap();
    let root = initial_prior_root();
    let mut step = signed.step.clone();
    step.core30_prior_root = Some(root);
    let debit = step.evaluate(SigmaRelation::SEND);
    assert!(debit.is_honest());
    let supplied = payment_transcript(
        &signed.objects[1],
        &signed.objects[0],
        &debit.statement,
        Fp::from(5),
        Fp::from(7),
    );
    let before = crate::admin_sigma::StateWitness::from(&receiver);
    verify_receiver(
        &signed,
        &receiver,
        &debit.statement,
        &request,
        &credential,
        &supplied,
        root,
        candidate_head(&send_objects::state(&before), root),
    );
}
