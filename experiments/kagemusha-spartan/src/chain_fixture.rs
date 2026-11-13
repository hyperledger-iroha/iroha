//! Explicit deterministic research chain; never a production key/custody path.
//!
//! Starts both wallets at the canonical zero Bootstrap. The sender receives an
//! ordinary Load100, then irrevocably Sends10 against a real receiver-signed
//! Request. All heads, roots, sequences, originals and receipt signatures are
//! derived from those exact transitions. The supplied public profiles are
//! expected test inputs, not sealed recursive-verification authority.

use crate::{
    bootstrap::{self, Context, Inputs, Receipt},
    credential::Credential,
    fee_schedule::SignedFeeSchedule,
    full::{self, WalletKey},
    indexed_map,
    ipa::Gens,
    load, monetary_state,
    request::{self, CompletedRequest},
    send, send_arithmetic,
    send_controls::QuotaWitness,
    send_maps, statement,
};
use ff::{Field, PrimeField};
use iroha_pasta::{Eq, Fp, poseidon};

pub struct Chain {
    pub sender_context: Context,
    pub receiver_context: Context,
    pub sender_key: WalletKey,
    pub receiver_key: WalletKey,
    pub sender_bootstrap: Inputs,
    pub receiver_bootstrap: Inputs,
    pub load: load::Witness,
    pub send: send::Witness,
    pub after_load_public: [Fp; 64],
    pub after_send_public: [Fp; 64],
}

impl Chain {
    pub fn payment_originals(&self) -> crate::payment_wire::Originals {
        crate::payment_wire::Originals {
            payer: self.send.credential.clone(),
            certificate: self.sender_bootstrap.certificate.clone(),
            request: self.send.request.clone(),
            statement: self.send.statement,
            receipt: self.send.receipt.clone(),
            policy_epoch: u64::from_le_bytes(
                self.after_send_public[33].to_repr()[..8]
                    .try_into()
                    .unwrap(),
            ),
            pending_root: self.after_send_public[36],
            credit_root: self.after_send_public[37],
        }
    }
}

fn hash(domain: &[u8; 8], values: &[Fp]) -> Fp {
    poseidon::hash_with_domain(u64::from_le_bytes(*domain), values)
}

/// Fixture-only software signer. Production preparation passes the exact
/// resulting body to an app-owned nonexportable hardware payment key.
pub fn own_receipt(
    key: &WalletKey,
    credential: &Credential,
    fields: &[Fp; 26],
    payment: Fp,
) -> Receipt {
    let mut receipt = Receipt {
        body: [0; 338],
        signature: [0; 64],
    };
    receipt.body[0] = 1;
    for (to, from) in [(2, 2), (34, 66), (66, 259)] {
        receipt.body[to..to + 32].copy_from_slice(&credential.body[from..from + 32]);
    }
    receipt.body[98..114].copy_from_slice(&fields[9].to_repr()[..16]);
    let limbs = |start| {
        crate::certificate::fp(&num_bigint::BigUint::from_bytes_le(
            &credential.body[start..start + 16],
        ))
    };
    let operation = hash(b"kgwopid1", &[limbs(66), limbs(82), fields[16], fields[17]]);
    for (to, value) in [
        (114, operation),
        (146, fields[14]),
        (178, fields[15]),
        (210, statement::digest(fields)),
        (242, Fp::from(987)),
        (306, payment),
    ] {
        receipt.body[to..to + 32].copy_from_slice(&value.to_repr());
    }
    receipt.body[274..306].fill(0x55);
    receipt.signature = bootstrap::sign(
        crate::receipt::message(&receipt),
        &key.d,
        "research chain own Advance receipt",
    );
    receipt
}

/// Reconstruct expected successor public ports. These are never trusted until
/// the corresponding generated proof is accepted by its linked verifier.
pub fn public_after(
    prior: &[Fp; 64],
    fields: &[Fp; 26],
    receipt: &Receipt,
    lineage: [Fp; 6],
    kind: usize,
) -> [Fp; 64] {
    let mut public = *prior;
    public[full::HEAD_IO] = fields[15];
    public[full::STATEMENT_IO] = statement::digest(fields);
    public[full::RECEIPT_IO] = crate::receipt::digest(receipt);
    public[32..38].copy_from_slice(&lineage);
    let (a, b) = crate::deferral::kind_bits(kind);
    public[full::KIND_IO] = Fp::from(a);
    public[full::KIND_IO + 1] = Fp::from(b);
    public
}

pub fn new(gens: &Gens<Eq>) -> Chain {
    new_inner(gens, None)
}

pub fn with_relation(gens: &Gens<Eq>, relation: [u8; 32]) -> Chain {
    new_inner(gens, Some(relation))
}

fn new_inner(gens: &Gens<Eq>, relation: Option<[u8; 32]>) -> Chain {
    let sender_key = WalletKey::labelled("complete research chain sender");
    let receiver_key = WalletKey::labelled("complete research chain receiver");
    let (mut sender_context, mut sender_bootstrap) = bootstrap::fixture(&sender_key, gens);
    let (mut receiver_context, mut receiver_bootstrap) = bootstrap::fixture(&receiver_key, gens);
    if let Some(relation) = relation {
        for (context, input, key) in [
            (&mut sender_context, &mut sender_bootstrap, &sender_key),
            (
                &mut receiver_context,
                &mut receiver_bootstrap,
                &receiver_key,
            ),
        ] {
            context.relation = relation;
            for i in 0..2 {
                input.statement[1 + i] = crate::certificate::fp(
                    &num_bigint::BigUint::from_bytes_le(&relation[16 * i..16 * i + 16]),
                );
            }
            input.receipt.body[210..242]
                .copy_from_slice(&statement::digest(&input.statement).to_repr());
            input.receipt.signature = bootstrap::sign(
                crate::receipt::message(&input.receipt),
                &key.d,
                "research Bootstrap selected relation",
            );
        }
    }
    assert_eq!(sender_context.root.scheme, receiver_context.root.scheme);
    assert_eq!(sender_context.relation, receiver_context.relation);
    let boot_public = bootstrap::public_profile(&sender_context, &sender_bootstrap);
    let mut ordinary = [0; 282];
    ordinary[0] = 1;
    ordinary[2..98].copy_from_slice(&sender_bootstrap.credential.body[2..98]);
    ordinary[98..130].fill(0x45);
    ordinary[146..162].copy_from_slice(&100u128.to_le_bytes());
    ordinary[210..242].fill(0x56);
    ordinary[242] = 2;
    ordinary[250..282].copy_from_slice(&sender_bootstrap.credential.body[98..130]);
    let load_digest = load::ordinary_digest(&ordinary);
    let recovery_value = poseidon::hash_with_domain(
        load::RECOVERY_DOMAIN,
        &[Fp::ZERO, load_digest, Fp::from(100)],
    );
    let (recovery, recovery_root) =
        indexed_map::empty_insert(Fp::from_u128(1u128 << 127).double(), recovery_value);
    let mut loaded = sender_bootstrap.core;
    loaded[9] = Fp::from(100);
    loaded[11] = Fp::ONE;
    loaded[13] = Fp::ONE;
    loaded[19] = recovery_root;
    loaded[29] += Fp::ONE;
    let mut load_statement = sender_bootstrap.statement;
    load_statement[9] = Fp::ONE;
    load_statement[10] = Fp::ONE;
    load_statement[14] = boot_public[full::HEAD_IO];
    load_statement[15] = monetary_state::commitment(&loaded, &sender_bootstrap.rest);
    load_statement[16] = Fp::from(2);
    load_statement[17] = load_digest;
    load_statement[18] = Fp::ZERO;
    load_statement[19] = Fp::from(100);
    load_statement[20..].fill(Fp::ZERO);
    let load_receipt = own_receipt(
        &sender_key,
        &sender_bootstrap.credential,
        &load_statement,
        Fp::ZERO,
    );
    let load = load::Witness {
        before_core: sender_bootstrap.core,
        before_rest: sender_bootstrap.rest,
        after_core: loaded,
        after_rest: sender_bootstrap.rest,
        credential: sender_bootstrap.credential.clone(),
        ordinary,
        statement: load_statement,
        receipt: load_receipt,
        recovery,
    };
    let after_load_public = public_after(
        &boot_public,
        &load.statement,
        &load.receipt,
        boot_public[32..38].try_into().unwrap(),
        full::LOAD,
    );

    let mut request = CompletedRequest {
        body: [0; 418],
        signature: [0; 64],
    };
    request.body[0] = 1;
    request.body[2..130].copy_from_slice(&sender_bootstrap.credential.body[2..130]);
    request.body[130..194].copy_from_slice(&receiver_bootstrap.credential.body[66..130]);
    request.body[194..210].copy_from_slice(&loaded[12].to_repr()[..16]);
    request.body[210..242]
        .copy_from_slice(&crate::credential::digest(&receiver_bootstrap.credential).to_repr());
    request.body[242..258].copy_from_slice(&10u128.to_le_bytes());
    request.body[258..290].copy_from_slice(&sender_bootstrap.rest[2].to_repr());
    request.body[306..314].copy_from_slice(&loaded[27].to_repr()[..8]);
    request.body[314..346].copy_from_slice(&sender_bootstrap.rest[1].to_repr());
    request.body[346..354].copy_from_slice(&110u64.to_le_bytes());
    request.body[354] = 1;
    request.body[386..418].fill(0x66);
    let message = request::commitments(&request).unwrap().message;
    request.signature = bootstrap::sign(
        message,
        &receiver_key.d,
        "research chain receiver-signed Request",
    );
    let terms = request::commitments(&request).unwrap();
    let mut send_statement = load.statement;
    send_statement[9] = loaded[11] + Fp::ONE;
    send_statement[10] = loaded[13];
    send_statement[11] = loaded[22];
    send_statement[12] = after_load_public[35];
    send_statement[13] = after_load_public[36];
    send_statement[14] = after_load_public[full::HEAD_IO];
    send_statement[16] = Fp::from(3);
    send_statement[17] = terms.credit;
    send_statement[18..20].copy_from_slice(&receiver_bootstrap.core[5..7]);
    send_statement[20] = loaded[12];
    send_statement[21] = Fp::from(10);
    send_statement[22] = Fp::ZERO;
    send_statement[23] = terms.digest;
    send_statement[24] = Fp::from(110);
    send_statement[25] = Fp::from(120);
    let pending_value =
        poseidon::hash_with_domain(send_maps::PENDING_DOMAIN, &send_statement[17..24]);
    let (pending, pending_root) = indexed_map::empty_insert(terms.credit, pending_value);
    let fee_value =
        poseidon::hash_with_domain(send_maps::FEE_DOMAIN, &[terms.credit, Fp::ZERO, Fp::ZERO]);
    let (fee, _) = indexed_map::empty_insert(terms.credit, fee_value);
    let mut sent = loaded;
    sent[9] -= Fp::from(10);
    sent[10] = after_load_public[35];
    sent[11] += Fp::ONE;
    sent[12] += Fp::ONE;
    sent[15] = poseidon::hash_with_domain(
        send_arithmetic::CHAIN_DOMAIN,
        &[
            loaded[15],
            terms.credit,
            send_statement[18],
            send_statement[19],
            loaded[12],
            Fp::from(10),
            Fp::ZERO,
            terms.digest,
        ],
    );
    sent[18] = pending_root;
    sent[28] = Fp::from(110);
    sent[29] += Fp::ONE;
    send_statement[15] = monetary_state::commitment(&sent, &load.after_rest);
    let send_receipt = own_receipt(&sender_key, &load.credential, &send_statement, Fp::ZERO);
    let send = send::Witness {
        before_core: loaded,
        before_rest: load.after_rest,
        after_core: sent,
        after_rest: load.after_rest,
        credential: load.credential.clone(),
        request,
        fee_schedule: SignedFeeSchedule {
            body: [0; 191],
            signature: [0; 64],
        },
        quota: QuotaWitness::default(),
        lower: 110,
        upper: 120,
        statement: send_statement,
        receipt: send_receipt,
        maps: send_maps::Witness { pending, fee },
    };
    let after_send_public = public_after(
        &after_load_public,
        &send.statement,
        &send.receipt,
        [
            sent[0],
            sent[27],
            sent[22],
            after_load_public[35],
            pending_root,
            after_load_public[37],
        ],
        full::SEND,
    );
    Chain {
        sender_context,
        receiver_context,
        sender_key,
        receiver_key,
        sender_bootstrap,
        receiver_bootstrap,
        load,
        send,
        after_load_public,
        after_send_public,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{operation_auth::tests as harness, send_arithmetic::Source};

    #[test]
    fn zero_bootstrap_load_send_chain_preserves_exact_predecessors_and_signed_request() {
        let chain = new(&Gens::<Eq>::new(1 << 16));
        assert_eq!(chain.sender_bootstrap.core[9], Fp::ZERO);
        assert_eq!(chain.receiver_bootstrap.core[9], Fp::ZERO);
        assert_eq!(chain.load.after_core[9], Fp::from(100));
        assert_eq!(chain.send.after_core[9], Fp::from(90));
        assert_eq!(chain.load.after_core, chain.send.before_core);
        assert_eq!(chain.load.after_rest, chain.send.before_rest);
        let mut w = full::Wiring::default();
        let family = w.family(0, 1);
        let boot = bootstrap::public_profile(&chain.sender_context, &chain.sender_bootstrap);
        let prior = harness::predecessor(&mut w, family, boot);
        let loaded = load::append(&mut w, family, &chain.load, &prior).unwrap();
        harness::expose(
            &mut w,
            family,
            4,
            &[
                loaded.head(),
                Source {
                    array: loaded.statement().local,
                    cell: loaded.statement().digest,
                    instance: 0,
                },
                loaded.receipt().digest(),
            ],
        );
        harness::expose(&mut w, family, 16, &loaded.context());
        harness::expose(&mut w, family, 32, &loaded.lineage());
        let built = harness::finish(w, 16);
        assert!(harness::holds(&built));
        assert_eq!(&built.1.io[4..7], &chain.after_load_public[4..7]);
        assert_eq!(&built.1.io[16..38], &chain.after_load_public[16..38]);
        let mut w = full::Wiring::default();
        let family = w.family(0, 1);
        let prior = harness::predecessor(&mut w, family, chain.after_load_public);
        let sent = send::append(&mut w, family, &chain.send, &prior).unwrap();
        harness::expose(
            &mut w,
            family,
            4,
            &[
                sent.head(),
                Source {
                    array: sent.statement().local,
                    cell: sent.statement().digest,
                    instance: 0,
                },
                sent.receipt().digest(),
            ],
        );
        harness::expose(&mut w, family, 16, &sent.context());
        harness::expose(&mut w, family, 32, &sent.lineage());
        let built = harness::finish(w, 16);
        assert!(harness::holds(&built));
        assert_eq!(&built.1.io[4..7], &chain.after_send_public[4..7]);
        assert_eq!(&built.1.io[16..38], &chain.after_send_public[16..38]);
        let message = request::commitments(&chain.send.request).unwrap().message;
        let z =
            num_bigint::BigUint::from_bytes_be(&crate::p256::sha256_one_block(&message.to_repr()));
        let signature = crate::p256::Signature {
            r: num_bigint::BigUint::from_bytes_be(&chain.send.request.signature[..32]),
            s: num_bigint::BigUint::from_bytes_be(&chain.send.request.signature[32..]),
        };
        assert!(crate::p256::verify(&chain.receiver_key.q, &z, &signature));
        assert!(!crate::p256::verify(&chain.sender_key.q, &z, &signature));
    }

    #[test]
    fn preparation_checks_exact_body_and_money_before_any_hardware_signature() {
        let chain = new(&Gens::<Eq>::new(1 << 16));
        let prepare_send = |input: &send::Witness| {
            let mut w = full::Wiring::default();
            let family = w.family(0, 1);
            let prior = harness::predecessor(&mut w, family, chain.after_load_public);
            let prepared = send::append_preparation(&mut w, family, input, &prior).unwrap();
            assert_eq!(prepared.body().body(), &input.receipt.body);
            harness::expose(
                &mut w,
                family,
                4,
                &[prepared.head(), prepared.body().message()],
            );
            harness::finish(w, 16)
        };
        let mut send = chain.send.clone();
        send.receipt.signature.fill(0);
        assert!(
            harness::holds(&prepare_send(&send)),
            "preparation does not forge or require a signature"
        );
        let mut w = full::Wiring::default();
        let family = w.family(0, 1);
        let prior = harness::predecessor(&mut w, family, chain.after_load_public);
        assert!(
            send::append(&mut w, family, &send, &prior).is_err(),
            "complete Send still requires actual valid signature"
        );
        send.after_core[9] += Fp::ONE;
        assert!(
            !harness::holds(&prepare_send(&send)),
            "preparation cannot spend an unchecked successor"
        );
        send = chain.send.clone();
        send.receipt.body[178] ^= 1;
        assert!(
            !harness::holds(&prepare_send(&send)),
            "preparation signs the exact constrained new head"
        );

        let prepare_load = |input: &load::Witness| {
            let mut w = full::Wiring::default();
            let family = w.family(0, 1);
            let boot = bootstrap::public_profile(&chain.sender_context, &chain.sender_bootstrap);
            let prior = harness::predecessor(&mut w, family, boot);
            let prepared = load::append_preparation(&mut w, family, input, &prior).unwrap();
            assert_eq!(prepared.body().body(), &input.receipt.body);
            harness::expose(
                &mut w,
                family,
                4,
                &[prepared.head(), prepared.body().message()],
            );
            harness::finish(w, 16)
        };
        let mut load = chain.load.clone();
        load.receipt.signature.fill(0);
        assert!(harness::holds(&prepare_load(&load)));
        load.ordinary[146] ^= 1;
        assert!(
            !harness::holds(&prepare_load(&load)),
            "unsigned Load preparation binds exact ordinary amount"
        );
        load = chain.load.clone();
        load.receipt.body[114] ^= 1;
        assert!(
            !harness::holds(&prepare_load(&load)),
            "unsigned Load preparation binds exact operation ID"
        );
    }
}
