//! Checked specialization of the receiver's original quote into the retained Payment.
use super::*;
use crate::{
    credential_policy,
    operation_auth::CredentialOpening,
    receive_originals,
    request::{self, CompletedRequest, word},
    signature,
};

// 482 original bytes packed in <= 31-byte words are injective in the Pasta field.
const PACKED: usize = (request::BODY_BYTES + request::SIGNATURE_BYTES).div_ceil(31);
pub(super) struct RequestResults {
    pub(super) signatures: Source,
    pub(super) open: [Source; PACKED],
    pub(super) completed: [Source; PACKED],
}

pub(super) fn append(
    w: &mut Wiring<Fp>,
    family: usize,
    input: &CompletedRequest,
    quoted: &CredentialOpening,
    key: signature::TotalKey<'_>,
    descriptor: [Source; 7],
    receipt_signature: Source,
) -> RequestResults {
    let original = receive_originals::append_request(w, family, input);
    let req = original.request();
    let at = |cell| source(req.local, cell);
    let word = |i| at(req.words[i]);
    let bytes = std::array::from_fn(|i| original.byte(request::BODY_BYTES + i));
    let signature = signature::append_signature_total(
        w,
        family,
        "ArchiveSent receiver signature on retained Request terms",
        at(req.message),
        &bytes,
        key,
        vec![],
    )
    .valid();
    let mut c = Cells::new(family);
    c.a.name = "ArchiveSent Request exact receiver, payer specialization and Send descriptor";
    // Both immutable record sources are fixed. A wrong Request cannot elect a no-op.
    c.eq(original.valid().linear(), constant(vec![Fp::ONE]));
    for (port, expected) in [
        (at(req.credit), descriptor[0]),
        (word(word::RECEIVER_WALLET[0]), descriptor[1]),
        (word(word::RECEIVER_WALLET[1]), descriptor[2]),
        (word(word::ORDINAL), descriptor[3]),
        (word(word::AMOUNT), descriptor[4]),
        (word(word::FEE), descriptor[5]),
        (at(req.digest), descriptor[6]),
        (word(word::RECEIVER_CREDENTIAL), quoted.digest()),
    ] {
        c.eq(port.linear(), expected.linear());
    }
    for (indices, expected) in [
        (word::SCHEME, quoted.scheme()),
        (word::ASSET, quoted.asset()),
        (word::RECEIVER_WALLET, quoted.wallet()),
        (word::RECEIVER_ACCOUNT, quoted.account()),
    ] {
        for (i, value) in indices.into_iter().zip(expected) {
            c.eq(word(i).linear(), value.linear());
        }
    }
    // The open form replaces only the payer wallet, payer account and Send ordinal
    // with zero. All other bytes, including the 64 signature bytes, share their sources.
    let pack = |c: &mut Cells, open: bool| -> [usize; PACKED] {
        std::array::from_fn(|chunk| {
            let mut weight = Fp::ONE;
            let mut sum = Fp::ZERO;
            let mut terms = vec![];
            let end = ((chunk + 1) * 31).min(request::BODY_BYTES + request::SIGNATURE_BYTES);
            for i in chunk * 31..end {
                if !open || !((66..130).contains(&i) || (194..210).contains(&i)) {
                    let byte = original.byte(i);
                    sum += byte.value(w) * weight;
                    terms = plus(&terms, &scaled(&byte.linear(), weight));
                }
                weight *= Fp::from(256);
            }
            let k = c.cell(sum, 255);
            c.eq(own(k, 1), terms);
            k
        })
    };
    let open = pack(&mut c, true);
    let completed = pack(&mut c, false);
    let bits = [receipt_signature, signature].map(|v| {
        let k = c.bit(v.value(w) == Fp::ONE);
        c.eq(own(k, 1), v.linear());
        k
    });
    let valid = credential_policy::all(&mut c, &bits);
    let local = w.push(c.a);
    RequestResults {
        signatures: source(local, valid),
        open: open.map(|k| source(local, k)),
        completed: completed.map(|k| source(local, k)),
    }
}

pub(super) fn expected_words(bytes: &[u8]) -> Vec<Fp> {
    assert_eq!(bytes.len(), request::BODY_BYTES + request::SIGNATURE_BYTES);
    bytes
        .chunks(31)
        .map(|bytes| certificate::fp(&num_bigint::BigUint::from_bytes_le(bytes)))
        .collect()
}
