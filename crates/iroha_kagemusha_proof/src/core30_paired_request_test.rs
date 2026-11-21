//! One ordinary candidate Send and Receive over the same signed Request.
//! The rooted path supplies authenticated payer funding and independent receiver enrollment.
use super::*;
use crate::omega_boundary_tests::RootedBootstrapOmega;
use crate::{
    a_relation::transition::core30_inline_receive_accepts, admin_sigma::StateWitness,
    operation_relation::objects::ObjectKind,
};
use ff::{Field, PrimeField};
use iroha_plonk_gadgets::statement::STATEMENT_DOMAIN;

/// Verify the original's low-S signature using the uncompressed payment key in the
/// quoted recipient credential. Neither the nonce nor the signing key is an input.
fn verify_request_original(raw: &[u8], credential: &[u8]) -> Option<bootstrap_objects::Signed> {
    if credential.len() != ObjectKind::Credential.body_len() + 64 {
        return None;
    }
    verify_signed_original(ObjectKind::Request, raw, &credential[130..195])
}

pub(super) fn verify_signed_original(
    kind: ObjectKind,
    raw: &[u8],
    sec1: &[u8],
) -> Option<bootstrap_objects::Signed> {
    use crate::operation_relation::objects::signed_message_native;
    use iroha_plonk_gadgets::{
        p256::native::{Affine, verify_prehashed, words_from_be},
        sha256::native::sha256_of_digest,
    };
    let end = kind.body_len();
    if raw.len() != end + 64 || sec1.len() != 65 || sec1[0] != 4 {
        return None;
    }
    let words = |bytes: &[u8]| Some(words_from_be(bytes.try_into().ok()?));
    let key = Affine {
        x: words(&sec1[1..33])?,
        y: words(&sec1[33..65])?,
    };
    let r = words(&raw[end..end + 32])?;
    let s = words(&raw[end + 32..])?;
    let message = signed_message_native(kind, &raw[..end]);
    let hash = words_from_be(&sha256_of_digest(&message));
    verify_prehashed(&hash, &r, &s, &key).then(|| bootstrap_objects::Signed {
        kind,
        bytes: raw.to_vec(),
        signature: crate::q_signature::SignatureWitness {
            digest: message,
            key: [key.x, key.y],
            signature: [r, s],
        },
    })
}

#[test]
#[ignore = "proves two Core30 steps and their incoming-only Q, and checks the matched recipient A1"]
fn signed_request_binds_paired_steps_incoming_q_and_inline_receive() {
    let (payer, _, payer_credential) = bootstrap_objects::enrollment();
    let (loaded, _, _) = load_objects::funded(&payer);
    check_matching_steps(&payer, &loaded.successor, &payer_credential, None, None);
}

pub(super) fn check_matching_steps(
    payer: &crate::admin_sigma::BootstrapWitness,
    sender_before: &StateWitness,
    payer_credential: &bootstrap_objects::Signed,
    funded_head: Option<Fp>,
    roots: Option<(&RootedBootstrapOmega, &RootedBootstrapOmega)>,
) {
    let send = send_objects::from_load(sender_before);
    let (enrolled, _, credential) = receive_objects::receiver();
    let recipient = roots.map_or(&enrolled, |(_, receiver)| &receiver.source.state);
    let (mut receive, _) =
        receive_objects::monetary_step(&send, &StateWitness::from(recipient), true);
    let request_body = send.step.request_body();
    assert_eq!(request_body, receive.request_body());
    assert_eq!(
        request_body.payer_wallet,
        send.step.predecessor.core.identity.wallet_id
    );
    assert_eq!(
        request_body.receiver_wallet,
        receive.predecessor.core.identity.wallet_id
    );
    assert_eq!(
        request_body.receiver_credential_digest,
        credential.digest().to_repr()
    );
    assert_eq!(send.objects[0], payer_credential.bytes);

    // The recipient signs the open terms. The actual monetary steps select the
    // payer wallet, account and ordinal and must agree with the entire instantiated original.
    assert_eq!(
        &send.objects[1][..ObjectKind::Request.body_len()],
        send_objects::request_body_bytes(&request_body)
    );
    let signed_request = verify_request_original(&send.objects[1], &credential.bytes).unwrap();
    let mut modified_signature = send.objects[1].clone();
    *modified_signature.last_mut().unwrap() ^= 1;
    assert!(verify_request_original(&modified_signature, &credential.bytes).is_none());
    assert!(verify_request_original(&send.objects[1], &payer_credential.bytes).is_none());
    let root = crate::admin_sigma::core30_initial::initial_prior_root();
    let mut payer_step = send.step;
    payer_step.core30_prior_root = Some(root);
    receive.core30_prior_root = Some(root);
    let debit = payer_step.evaluate(SigmaRelation::SEND);
    if let Some(proved_funded_head) = funded_head {
        assert_eq!(debit.statement[14], proved_funded_head);
    }
    let credit = receive.evaluate(SigmaRelation::RECEIVE);
    if let Some((payer_root, receiver_root)) = roots {
        verify_rooted_receive_predecessor(
            payer_root,
            receiver_root,
            &receive,
            &credit.statement,
            &credential,
        );
    }
    assert!(debit.is_honest() && credit.is_honest());
    let amount = request_body.terms.amount;
    let fee = request_body.terms.fee;
    assert_eq!(debit.statement[17], request_body.credit_id());
    assert_eq!(credit.statement[17], debit.statement[17]);
    assert_eq!(debit.statement[18..20], recipient.core[5..7]);
    assert_eq!(credit.statement[18..20], payer.core[5..7]);
    assert_eq!(debit.statement[21], Fp::from_u128(amount));
    assert_eq!(credit.statement[20], debit.statement[21]);
    assert_eq!(debit.statement[22], Fp::from_u128(fee));
    assert_eq!(debit.statement[23], signed_request.digest());
    let after_debit = debit.successor_state.as_ref().unwrap();
    let after_credit = credit.successor_state.as_ref().unwrap();
    assert_eq!(
        after_debit.core.balance + amount + fee,
        payer_step.predecessor.core.balance
    );
    assert_eq!(
        after_credit.core.balance,
        receive.predecessor.core.balance + amount
    );
    for (step, successor, statement) in [
        (&payer_step, after_debit, &debit.statement),
        (&receive, after_credit, &credit.statement),
    ] {
        assert_eq!(statement[14], candidate_head(&step.predecessor, root));
        assert_eq!(statement[15], candidate_head(successor, root));
    }
    // Prove both exact statements and bind the Send original to the Q opening.
    // The prover derives each statement independently from the ordinary witness.
    payer_step.core30_prior_root = None;
    let mut ordinary_receive = receive.clone();
    ordinary_receive.core30_prior_root = None;
    let (payer_verifier, payer_proof, ordinary_payer_public) =
        prove_candidate_step(SigmaRelation::SEND, payer_step, root);
    let (recipient_verifier, recipient_proof, _) =
        prove_candidate_step(SigmaRelation::RECEIVE, ordinary_receive, root);
    assert_eq!(payer_proof.public, debit.public());
    assert_eq!(recipient_proof.public, credit.public());
    let started = Instant::now();
    assert!(core30_inline_receive_accepts(&receive, credit.statement));
    let inline = started.elapsed();

    // A different Request can itself give rise to a locally honest Receive.
    // Its credit identifier, amount and statement must fail against the original proof and A1.
    let mut substituted = receive.clone();
    let crate::StepInputs::Receive(input) = &mut substituted.inputs else {
        unreachable!()
    };
    input.request.amount += 1;
    let other = substituted.evaluate(SigmaRelation::RECEIVE);
    assert!(other.is_honest());
    assert_ne!(other.statement[17], debit.statement[17]);
    assert_ne!(other.statement[20], debit.statement[21]);
    assert!(
        recipient_verifier
            .verify(&other.public(), &recipient_proof.bytes)
            .is_err()
    );
    assert!(!core30_inline_receive_accepts(
        &substituted,
        credit.statement
    ));
    let mut other_debit = debit.statement;
    other_debit[23] += Fp::ONE;
    let other_payer_public = StepPublic {
        statement: hash_with_domain(STATEMENT_DOMAIN, &other_debit),
    };
    assert!(
        payer_verifier
            .verify(&other_payer_public, &payer_proof.bytes)
            .is_err()
    );

    prove_incoming_q(
        SigmaRelation::SEND,
        payer_verifier,
        payer_proof,
        ordinary_payer_public,
        2,
        Instant::now(),
    );
    println!(
        "CORE30_PAIRED_REQUEST amount={amount} fee={fee} signed_request_bytes={} credit={:?} inline_receive_strict_ms={:.3} sender_omega=false advance=false",
        signed_request.bytes.len(),
        credit.statement[17],
        inline.as_secs_f64() * 1e3
    );
}

/// Decide the receiver's exact predecessor in the accepted one-terminal catalog.
/// The statement comes from the same ordinary transition passed to sigma and A1.
fn verify_rooted_receive_predecessor(
    payer: &RootedBootstrapOmega,
    receiver: &RootedBootstrapOmega,
    receive: &crate::StepWitness<Fp>,
    statement: &[Fp; 26],
    credential: &bootstrap_objects::Signed,
) {
    use iroha_pasta::{Ep, msm::MemoryBudget};
    use iroha_plonk::{pcs::ipa::PinnedParams, verify_full};
    use iroha_plonk_recursion::FoldInput;

    let started = Instant::now();
    let state = &receiver.source.state;
    let mut fields = state.lineage;
    let wallet = receive.predecessor.core.identity.wallet_id;
    assert_eq!(
        bootstrap_objects::id(fields[6], fields[7]).as_slice(),
        wallet.as_slice()
    );
    assert_eq!(&credential.bytes[66..98], wallet.as_slice());
    assert_eq!(credential.digest(), state.core[7]);
    assert_eq!(fields[8], credential.digest());
    let payment_key = &credential.bytes[130..195];
    assert_eq!(payment_key[0], 4);
    for (coordinate, words) in payment_key[1..]
        .chunks_exact(32)
        .zip(fields[9..13].chunks_exact(2))
    {
        let high = u128::from_be_bytes(coordinate[..16].try_into().unwrap());
        let low = u128::from_be_bytes(coordinate[16..].try_into().unwrap());
        assert_eq!(words, [Fp::from_u128(low), Fp::from_u128(high)]);
    }
    assert_eq!(state.core[8], Fp::ZERO);
    assert_eq!(fields[5], statement[14]);
    assert_eq!(
        fields[17],
        payer.key.kagemusha_digest(&payer.binding).unwrap()
    );

    let pallas = FoldInput::from_opening(
        *receiver.source.pallas.g(),
        receiver.source.pallas.challenges(),
    )
    .unwrap();
    let public = |fields: &[Fp; 18]| {
        let digest = crate::bootstrap_owner_fixture::digest(fields, &pallas);
        crate::fold_wrap::native::omega_instances(digest, &receiver.vesta)
    };
    let expected = public(&fields);
    assert_eq!(expected, receiver.instances);
    let independent = PinnedParams::<Ep>::derive(16).unwrap();
    let accepts = |instances: &[_], proof: &[u8]| {
        verify_full(
            &independent,
            &payer.binding,
            &payer.key,
            instances,
            proof,
            MemoryBudget::DEFAULT,
        )
        .is_ok()
    };
    assert!(accepts(&expected, &receiver.proof));
    // A valid enrollment under this same catalog, but for the paying wallet.
    assert!(!accepts(&expected, &payer.proof));

    // A different prior-credential root still permits an honest ordinary
    // transition and the same signed payment. Its head has no such enrollment.
    let mut changed = receive.clone();
    changed.core30_prior_root = Some(receive.core30_prior_root.unwrap() + Fp::ONE);
    let other = changed.evaluate(SigmaRelation::RECEIVE);
    assert!(other.is_honest());
    assert_eq!(other.statement[17], statement[17]);
    assert_ne!(other.statement[14], statement[14]);
    fields[5] = other.statement[14];
    assert!(!accepts(&public(&fields), &receiver.proof));
    assert!(!core30_inline_receive_accepts(&changed, *statement));
    println!(
        "CORE30_ROOTED_RECEIVER_BOOTSTRAP omega_bytes={} scoped_verification_and_refusals_ms={:.3} head={:?} credit={:?} payer_substitution_refused=true alternate_prior_root_refused=true",
        receiver.proof.len(),
        started.elapsed().as_secs_f64() * 1e3,
        statement[14],
        statement[17],
    );
}
