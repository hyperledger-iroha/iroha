//! Deterministic public research witness for the first Receive after Bootstrap.
//!
//! This constructs witness material only. It does not authenticate a proof,
//! admit a Payment, use production custody keys or create spendable value.
use crate::{
    bootstrap,
    certificate::fp,
    full::WalletKey,
    indexed_map::{self, SearchWitness},
    monetary_state, payment, receive, receive_lineage as lineage,
    request::{self, word},
    send,
};
use ff::{Field, PrimeField};
use iroha_pasta::{Fp, poseidon};
use num_bigint::BigUint;

pub fn from_send(
    receiver: &bootstrap::Inputs,
    receiver_key: &WalletKey,
    send: &send::Witness,
    pair_bytes: &[u8],
    prior_public: [Fp; 64],
) -> Result<receive::Witness, &'static str> {
    from_send_with_validity(receiver, receiver_key, send, pair_bytes, prior_public, true)
}

/// Warm-up research circuits may receive an incoming pair whose deferred
/// accumulator belongs to the preceding program. Its exact derived verdict
/// selects the burned credit value; final wallet-chain tests require true.
pub fn from_send_with_validity(
    receiver: &bootstrap::Inputs,
    receiver_key: &WalletKey,
    send: &send::Witness,
    pair_bytes: &[u8],
    prior_public: [Fp; 64],
    incoming_valid: bool,
) -> Result<receive::Witness, &'static str> {
    let rq = request::commitments(&send.request)?;
    let payment = payment::digests(
        &send.request,
        &send.credential,
        &send.statement,
        &send.receipt,
        pair_bytes,
    )?;
    let before = receiver.core;
    let rest = receiver.rest;
    if prior_public[crate::full::HEAD_IO] != monetary_state::commitment(&before, &rest) {
        return Err("Receive fixture predecessor head mismatch");
    }
    if rq.words[word::RECEIVER_CREDENTIAL] != crate::credential::digest(&receiver.credential) {
        return Err("Receive fixture requires current quoted credential");
    }
    let mut after = before;
    after[9] += rq.words[word::AMOUNT];
    after[11] += Fp::ONE;
    after[16] = poseidon::hash_with_domain(
        receive::CHAIN_DOMAIN,
        &[
            before[16],
            rq.credit,
            rq.words[word::PAYER_WALLET[0]],
            rq.words[word::PAYER_WALLET[1]],
            rq.words[word::AMOUNT],
        ],
    );
    after[29] += Fp::ONE;
    let consumed_value = poseidon::hash_with_domain(
        lineage::CONSUMED_DOMAIN,
        &[rq.credit, rq.words[word::AMOUNT], after[11]],
    );
    let (consumed_insert, consumed_root) =
        indexed_map::empty_insert::<32>(rq.credit, consumed_value);
    let credit_value = poseidon::hash_with_domain(
        lineage::CREDIT_DOMAIN,
        &[
            rq.credit,
            payment.payment,
            Fp::from(u64::from(!incoming_valid)),
        ],
    );
    let (credit_insert, _) = indexed_map::empty_insert::<32>(rq.credit, credit_value);
    if before[17] != consumed_insert.old_root || prior_public[37] != credit_insert.old_root {
        return Err("Receive fixture requires empty consumed and adjusted credit maps");
    }
    after[17] = consumed_root;
    let consumed_search = SearchWitness {
        root: consumed_insert.old_root,
        key: rq.credit,
        leaf: consumed_insert.low,
        path: consumed_insert.low_path.clone(),
    };
    let credit_search = SearchWitness {
        root: credit_insert.old_root,
        key: rq.credit,
        leaf: credit_insert.low,
        path: credit_insert.low_path.clone(),
    };
    let mut statement = receiver.statement;
    statement[8] = before[0];
    statement[9] = after[11];
    statement[10] = after[13];
    statement[11] = before[22];
    statement[12] = Fp::ZERO;
    statement[13] = Fp::ZERO;
    statement[14] = monetary_state::commitment(&before, &rest);
    statement[15] = monetary_state::commitment(&after, &rest);
    statement[16] = Fp::from(4);
    statement[17..].fill(Fp::ZERO);
    statement[17] = rq.credit;
    statement[18] = rq.words[word::PAYER_WALLET[0]];
    statement[19] = rq.words[word::PAYER_WALLET[1]];
    statement[20] = rq.words[word::AMOUNT];
    let operation = poseidon::hash_with_domain(
        u64::from_le_bytes(*b"kgwopid1"),
        &[before[5], before[6], Fp::from(4), rq.credit],
    );
    let mut receipt = receiver.receipt.clone();
    receipt.body[98..114].copy_from_slice(&after[11].to_repr()[..16]);
    for (at, v) in [
        (114, operation),
        (146, statement[14]),
        (178, statement[15]),
        (210, crate::statement::digest(&statement)),
        (306, payment.payment),
    ] {
        receipt.body[at..at + 32].copy_from_slice(v.to_repr().as_ref());
    }
    receipt.signature = bootstrap::sign(
        bootstrap::receipt_message(&receipt),
        &receiver_key.d,
        "research first Receive receipt",
    );
    let mut empty = poseidon::hash_with_domain(lineage::PRIOR_LEAF_DOMAIN, &[Fp::ZERO]);
    let siblings = std::array::from_fn(|_| {
        let r = empty;
        empty = poseidon::hash_with_domain(lineage::PRIOR_NODE_DOMAIN, &[empty, empty]);
        r
    });
    // Fixture arithmetic must fit the same native u128 domains as the circuit.
    for value in [after[9], after[11]] {
        if BigUint::from_bytes_le(value.to_repr().as_ref()).bits() > 128 {
            return Err("Receive fixture integer overflow");
        }
    }
    let expected_q = [
        fp(&(&receiver_key.q.x % (BigUint::from(1u64) << 128))),
        fp(&(&receiver_key.q.x >> 128)),
        fp(&(&receiver_key.q.y % (BigUint::from(1u64) << 128))),
        fp(&(&receiver_key.q.y >> 128)),
    ];
    if prior_public[25..29] != expected_q {
        return Err("Receive fixture key mismatch");
    }
    Ok(receive::Witness {
        before_core: before,
        before_rest: rest,
        after_core: after,
        after_rest: rest,
        credential: receiver.credential.clone(),
        request: send.request.clone(),
        payer: send.credential.clone(),
        quoted_receiver: receiver.credential.clone(),
        incoming_statement: send.statement,
        incoming_receipt: send.receipt.clone(),
        prior_credential: lineage::PriorCredentialOpening {
            digest: Fp::ZERO,
            siblings,
        },
        statement,
        receipt,
        maps: lineage::ReceiveMapWitness {
            consumed_search,
            consumed_insert,
            insert: true,
            credit_search,
            credit_insert,
        },
    })
}
