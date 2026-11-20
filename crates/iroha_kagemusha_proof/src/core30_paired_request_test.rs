//! One ordinary candidate Send and Receive over the same signed Request.
//! The payer's predecessor lineage and Advance are outside this component.
use super::*;
use crate::{
    a_relation::transition::core30_inline_receive_accepts, admin_sigma::StateWitness,
    operation_relation::objects::ObjectKind,
};
use ff::{Field, PrimeField};
use iroha_plonk_gadgets::statement::STATEMENT_DOMAIN;

/// Verify the original's low-S signature using the uncompressed payment key in the
/// quoted recipient credential. Neither the nonce nor the signing key is an input.
fn verify_request_original(raw: &[u8], credential: &[u8]) -> Option<bootstrap_objects::Signed> {
    use crate::operation_relation::objects::signed_message_native;
    use iroha_plonk_gadgets::{
        p256::native::{Affine, verify_prehashed, words_from_be},
        sha256::native::sha256_of_digest,
    };
    let kind = ObjectKind::Request;
    let end = kind.body_len();
    if raw.len() != end + 64
        || credential.len() != ObjectKind::Credential.body_len() + 64
        || credential[130] != 4
    {
        return None;
    }
    let words = |bytes: &[u8]| Some(words_from_be(bytes.try_into().ok()?));
    let key = Affine {
        x: words(&credential[131..163])?,
        y: words(&credential[163..195])?,
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
    let send = send_objects::from_load(&loaded.successor);
    let (recipient, _, credential) = receive_objects::receiver();
    let (mut receive, _) =
        receive_objects::monetary_step(&send, &StateWitness::from(&recipient), true);
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
    let mut root = hash_with_domain(u64::from_le_bytes(*b"kgwpcrd1"), &[Fp::ZERO]);
    for _ in 0..16 {
        root = hash_with_domain(u64::from_le_bytes(*b"kgwpcrn1"), &[root, root]);
    }
    let mut payer_step = send.step;
    payer_step.core30_prior_root = Some(root);
    receive.core30_prior_root = Some(root);
    let debit = payer_step.evaluate(SigmaRelation::SEND);
    let credit = receive.evaluate(SigmaRelation::RECEIVE);
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
