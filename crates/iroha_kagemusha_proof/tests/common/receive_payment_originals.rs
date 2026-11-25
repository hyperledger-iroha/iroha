// Shared byte transcripts for the retained native Receive and its Core30 predecessor fixture.
fn frame(bytes: &[u8]) -> Vec<u8> {
    let mut out = u32::try_from(bytes.len()).unwrap().to_le_bytes().to_vec();
    out.extend(bytes);
    out
}
pub(crate) fn public_bytes(fields: &[Fp; 18]) -> Vec<u8> {
    let mut out = 1u16.to_le_bytes().to_vec();
    for i in [1, 2, 3, 4] {
        out.extend(&fields[i].to_repr()[..16]);
    }
    out.extend(fields[5].to_repr());
    for i in [6, 7] {
        out.extend(&fields[i].to_repr()[..16]);
    }
    out.extend(fields[8].to_repr());
    out.push(4);
    for i in [10, 9, 12, 11] {
        out.extend(fields[i].to_repr()[..16].iter().rev());
    }
    out.extend(&fields[13].to_repr()[..13]);
    out.extend(&fields[14].to_repr()[..16]);
    out.extend(fields[15].to_repr());
    out.extend(fields[16].to_repr());
    assert_eq!(out.len(), 320);
    out
}
fn signed_digest(kind: ObjectKind, raw: &[u8]) -> Fp {
    use iroha_kagemusha_proof::operation_relation::objects::signed_message_native;
    let end = kind.body_len();
    let mut words = vec![signed_message_native(kind, &raw[..end])];
    for offset in [16, 0, 48, 32] {
        words.push(Fp::from_u128(u128::from_be_bytes(
            raw[end + offset..end + offset + 16].try_into().unwrap(),
        )));
    }
    hash_with_domain(kind.object_domain(), &words)
}
/// Build the transitive Payment from the retained payer credential. Request is
/// signed by the receiver, but Payment's key identifies the payer's Send.
fn payment_transcript(
    request: &[u8],
    payer: &[u8],
    statement: &[Fp; 26],
    proof_digest: Fp,
    receipt: Fp,
) -> Vec<u8> {
    let package = hash_with_domain(
        u64::from_le_bytes(*b"kgwpkg_1"),
        &[
            hash_with_domain(iroha_plonk_gadgets::statement::STATEMENT_DOMAIN, statement),
            proof_digest,
            receipt,
        ],
    );
    let mut payment = 1u16.to_le_bytes().to_vec();
    payment.extend(signed_digest(ObjectKind::Request, request).to_repr());
    payment.extend(&payer[130..195]);
    payment.extend(signed_digest(ObjectKind::Credential, payer).to_repr());
    payment.extend(package.to_repr());
    assert_eq!(payment.len(), 163);
    payment
}

fn receipt(
    statement: &[Fp; 26],
    wallet: &[Fp],
    proof: Fp,
    payment: Fp,
    secret: u64,
    nonce: u64,
) -> bootstrap_objects::Signed {
    let operation = hash_with_domain(
        u64::from_le_bytes(*b"kgwopid1"),
        &[wallet[0], wallet[1], statement[16], statement[17]],
    );
    let mut body = 1u16.to_le_bytes().to_vec();
    body.extend(bootstrap_objects::id(statement[3], statement[4]));
    body.extend(bootstrap_objects::id(wallet[0], wallet[1]));
    body.extend(bootstrap_objects::small_id(31, 32));
    body.extend(&statement[9].to_repr()[..16]);
    for word in [
        operation,
        statement[14],
        statement[15],
        hash_with_domain(iroha_plonk_gadgets::statement::STATEMENT_DOMAIN, statement),
        proof,
    ] {
        body.extend(word.to_repr());
    }
    body.extend(bootstrap_objects::small_id(501, 502));
    body.extend(payment.to_repr());
    bootstrap_objects::sign(ObjectKind::Receipt, body, secret, nonce)
}
