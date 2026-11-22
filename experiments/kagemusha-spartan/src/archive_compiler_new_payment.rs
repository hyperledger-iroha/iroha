//! Reconstruct the compact A→B Payment and B's canonical credit opening from
//! the actual compiler-generated proofs, signed transfers and adjusted maps.
use super::*;
use crate::native::{engine::credit_status_wire, sent_delivery::ReceiverEvidence};
use crate::{bootstrap::Receipt, credential::Credential, payment_wire};

#[derive(Clone)]
pub(in super::super) struct CompilerPayment {
    trusted: carrier::TrustedProfile,
    decided_original: blake3::Hash,
    pub(in super::super) exact: Vec<u8>,
    pub(in super::super) pair_bytes: Vec<u8>,
    pub(in super::super) digest: Fp,
    pub(in super::super) size: payment_wire::Sizes,
    pub(in super::super) originals: payment_wire::Originals,
    decisions: [bool; 4],
}

fn same_credential(a: &Credential, b: &Credential) -> bool {
    a.body == b.body && a.signature == b.signature
}

fn same_receipt(a: &Receipt, b: &Receipt) -> bool {
    a.body == b.body && a.signature == b.signature
}

fn generation(gens: &full::PairGens) -> native::Result<()> {
    if gens.defer_p.kinds.len() == 5 && gens.defer_s.kinds.len() == 5 {
        Ok(())
    } else {
        Err(Error::AuthorityMismatch("five-kind transfer generation"))
    }
}

fn scope(payer: &Inputs<'_>, params: &native::AuthenticatedParameters) -> native::Result<()> {
    payer.delivery.payment().require_parameters(params)
}

fn verify_signature(credential: &Credential, receipt: &Receipt) -> native::Result<()> {
    let key = native::point(&credential.body[130..195])?;
    crate::certificate::verify_native(&key, crate::receipt::message(receipt), &receipt.signature)
        .map_err(|_| Error::AuthorityMismatch("compiler signed transfer receipt"))
}

fn transport<'a>(
    params: &native::AuthenticatedParameters,
    pair: &'a carrier::TrustedProfile,
) -> payment_wire::Profile<'a> {
    payment_wire::Profile {
        pair,
        scheme: params.inner.scheme,
        relation: params.inner.relation,
    }
}

fn lineage(pair: &full::UniformPair) -> native::Result<[Fp; bc::LINEAGE_FIELDS]> {
    let profile: &[Fp; 64] = pair
        .ip
        .io
        .as_slice()
        .try_into()
        .map_err(|_| Error::Malformed("compiler monetary IO"))?;
    Ok(profile[bc::LINEAGE_IO..bc::LINEAGE_IO + bc::LINEAGE_FIELDS]
        .try_into()
        .unwrap())
}

pub(in super::super) fn new_payment(
    payer: &Inputs<'_>,
    params: &native::AuthenticatedParameters,
    signed: &crate::send::Witness,
    pair: &full::UniformPair,
    gens: &full::PairGens,
    programs: [[u8; 32]; 2],
) -> native::Result<CompilerPayment> {
    new_payment_candidate(payer, params, signed, pair, gens, programs)?.require_decided()
}

pub(in super::super) fn new_payment_candidate(
    payer: &Inputs<'_>,
    params: &native::AuthenticatedParameters,
    signed: &crate::send::Witness,
    pair: &full::UniformPair,
    gens: &full::PairGens,
    programs: [[u8; 32]; 2],
) -> native::Result<Candidate<CompilerPayment>> {
    generation(gens)?;
    payment_from_selected(payer, params, signed, pair, gens, programs)
}

fn payment_from_selected(
    payer: &Inputs<'_>,
    params: &native::AuthenticatedParameters,
    signed: &crate::send::Witness,
    pair: &full::UniformPair,
    gens: &full::PairGens,
    programs: [[u8; 32]; 2],
) -> native::Result<Candidate<CompilerPayment>> {
    scope(payer, params)?;
    let retained = payer.delivery.payment();
    let old_profile = params.inner.profile(full::SEND)?;
    let anchor = payment_wire::decode(&transport(params, &old_profile), retained.exact_bytes())
        .map_err(Error::Rejected)?
        .originals;
    if pair.kind != full::SEND
        || signed.request != anchor.request
        || !same_credential(&signed.credential, &anchor.payer)
        || !same_credential(
            &signed.request_originals.receiver,
            &payer.delivery.receiver_evidence().credential,
        )
    {
        return Err(Error::AuthorityMismatch(
            "compiler retained signed payer and Request",
        ));
    }
    verify_signature(&signed.credential, &signed.receipt)?;
    let lineage = lineage(pair)?;
    let originals = payment_wire::Originals {
        payer: signed.credential.clone(),
        certificate: anchor.certificate,
        request: signed.request.clone(),
        statement: signed.statement,
        receipt: signed.receipt.clone(),
        policy_epoch: funding::uint(lineage[bc::POLICY_EPOCH])?
            .try_into()
            .map_err(|_| Error::Malformed("compiler Payment policy epoch"))?,
        pending_root: lineage[bc::PENDING_ROOT],
        credit_root: lineage[bc::CREDIT_ROOT],
    };
    let trusted = carrier::TrustedProfile::from_parameters(
        params.inner.relation,
        pair.cp.shape,
        pair.cp.as_ref(),
        pair.cs.shape,
        pair.cs.as_ref(),
        true,
    )
    .map_err(Error::Rejected)?;
    let profile = transport(params, &trusted);
    let exact = payment_wire::encode(
        &profile, &originals, &pair.ip, &pair.is, &pair.pp, &pair.ps, pair.r,
    )
    .map_err(Error::Rejected)?;
    let decoded = payment_wire::decode(&profile, &exact).map_err(Error::Rejected)?;
    let candidate = checked_pair(params, pair, gens, programs, &decoded.projection)?;
    if candidate.original != decoded.pair_bytes {
        return Err(Error::AuthorityMismatch("compiler compact Payment pair"));
    }
    let decisions = candidate.decisions;
    Ok(Candidate {
        decisions,
        original: CompilerPayment {
            trusted,
            decided_original: blake3::hash(&exact),
            exact,
            pair_bytes: decoded.pair_bytes,
            digest: decoded.digests.payment,
            size: decoded.size,
            originals: decoded.originals,
            decisions,
        },
    })
}

pub(in super::super) fn new_receive_status(
    payer: &Inputs<'_>,
    params: &native::AuthenticatedParameters,
    payment: &CompilerPayment,
    signed: &crate::receive::Witness,
    pair: &full::UniformPair,
    gens: &full::PairGens,
    programs: [[u8; 32]; 2],
) -> native::Result<ScopedStatus> {
    if payment.decisions != [true; 4] {
        return Err(Error::AuthorityMismatch(
            "compiler incoming Payment accumulator decision",
        ));
    }
    new_receive_status_candidate(payer, params, payment, signed, pair, gens, programs)?
        .require_decided()
}

pub(in super::super) fn new_receive_status_candidate(
    payer: &Inputs<'_>,
    params: &native::AuthenticatedParameters,
    payment: &CompilerPayment,
    signed: &crate::receive::Witness,
    pair: &full::UniformPair,
    gens: &full::PairGens,
    programs: [[u8; 32]; 2],
) -> native::Result<Candidate<ScopedStatus>> {
    generation(gens)?;
    status_from_selected(payer, params, payment, signed, pair, gens, programs)
}

fn status_from_selected(
    payer: &Inputs<'_>,
    params: &native::AuthenticatedParameters,
    payment: &CompilerPayment,
    signed: &crate::receive::Witness,
    pair: &full::UniformPair,
    gens: &full::PairGens,
    programs: [[u8; 32]; 2],
) -> native::Result<Candidate<ScopedStatus>> {
    scope(payer, params)?;
    if blake3::hash(&payment.exact) != payment.decided_original {
        return Err(Error::AuthorityMismatch(
            "compiler Payment decision original",
        ));
    }
    // Decode the original under its producing Send tape geometry. A later closure
    // round can use another polynomial and new receiver tape geometry.
    let decoded = payment_wire::decode(&transport(params, &payment.trusted), &payment.exact)
        .map_err(Error::Rejected)?;
    let a = &decoded.originals;
    let quoted = &payer.delivery.receiver_evidence().credential;
    let digest = decoded.digests.payment;
    if decoded.pair_bytes != payment.pair_bytes {
        return Err(Error::AuthorityMismatch(
            "compiler exact retained Send pair",
        ));
    }
    if pair.kind != full::RECEIVE
        || signed.statement[16] != Fp::from(4)
        || signed.statement[17] != a.statement[17]
        || signed.request != a.request
        || a.request != *payer.delivery.payment().request()
        || !same_credential(&signed.payer, &a.payer)
        || !same_credential(&signed.quoted_receiver, quoted)
        || signed.incoming_statement != a.statement
        || !same_receipt(&signed.incoming_receipt, &a.receipt)
        || digest != payment.digest
        || signed.receipt.body[306..338] != digest.to_repr()
    {
        return Err(Error::AuthorityMismatch(
            "compiler Receive exact incoming Payment",
        ));
    }
    verify_signature(quoted, &signed.receipt)?;
    let lineage = lineage(pair)?;
    let (outcome, next, path) = credit_status_wire::receive_opening(
        &signed.maps,
        a.statement[17],
        digest,
        lineage[bc::CREDIT_ROOT],
    )?;
    let projection =
        credit_status_wire::projected(quoted, &signed.statement, &signed.receipt, lineage)?;
    let linked = checked_pair(params, pair, gens, programs, &projection)?;
    let evidence = ReceiverEvidence {
        credential: quoted.clone(),
        receipt: signed.receipt.clone(),
        statement: signed.statement,
        profile: projection.primary().io.as_slice().try_into().unwrap(),
        exact_pair: linked.original,
        pair: pair.clone(),
        next,
        slot: path.index,
        siblings: path.siblings,
    };
    let receipt = receiver_receipt_fields(&evidence, &a.statement, &a.request, digest, outcome)?;
    let status = evidence.wire_status(a.statement[17], digest, outcome)?;
    let status_digest = status
        .credit_status_digest()
        .map_err(|_| Error::AuthorityMismatch("compiler new CreditStatus"))?;
    let credited = credited_digest(a.statement[17], digest, status_digest)?;
    Ok(Candidate {
        decisions: linked.decisions,
        original: ScopedStatus {
            status,
            receipt,
            status_digest,
            credited,
            payment: digest,
        },
    })
}

#[test]
#[ignore = "reconstructs two signed retained payments and their native commitments"]
fn fresh_transfer_reconstruction_matches_signed_payment_and_credit_status() {
    with_retained_inputs(|payer, send, sent, engine, params, wallet| {
        let signed_witness = |p: &PreparedOperation, c: &CompletedOperation| {
            let mut wit = p.witness.as_ref().clone();
            let receipt = native::engine::receipt_mut(&mut wit);
            let bytes = c.completion().exact_receipt();
            assert_eq!(
                receipt.body.as_slice(),
                &bytes[..crate::receipt::BODY_BYTES]
            );
            receipt
                .signature
                .copy_from_slice(&bytes[crate::receipt::BODY_BYTES..]);
            wit
        };
        let pins = |kind: usize| {
            let program = &params.inner.programs[kind];
            [
                program.primary.program_digest(),
                program.secondary.program_digest(),
            ]
        };
        let gens = &params.inner.generators;
        let (_, _, _, _, send_pair) =
            native::engine::decode_head(sent.head().exact_pair(), params).unwrap();
        let native::ExactOperationWitness::Send(s) = &signed_witness(send, sent) else {
            panic!("Send")
        };
        assert!(new_payment(&payer, params, &s, &send_pair, gens, pins(full::SEND)).is_err());
        let payment = payment_from_selected(&payer, params, &s, &send_pair, gens, pins(full::SEND))
            .unwrap()
            .require_decided()
            .unwrap();
        let original = payer.delivery.payment();
        assert_eq!(payment.exact, original.exact_bytes());
        assert_eq!(payment.pair_bytes, original.exact_pair());
        assert_eq!(payment.digest, original.digest());
        assert_eq!(payment.originals.statement, *original.statement());
        let mut damaged = (*s).clone();
        damaged.receipt.signature[12] ^= 1;
        assert!(
            payment_from_selected(&payer, params, &damaged, &send_pair, gens, pins(full::SEND))
                .is_err()
        );

        let read = |name: &str| std::fs::read(wallet.join(name)).unwrap();
        let cap = native::RecoveryCapsule::decode(
            &read("counterparty-receive-capsule.norito"),
            &native::device_fixture::scheme().scheme_id(),
        )
        .unwrap();
        let completion = native::CompletionRecord::decode(
            &read("counterparty-receive-completion.norito"),
            cap.wallet(),
        )
        .unwrap();
        let (prepared, released) = engine
            .verify_released_operation(&cap, &completion, params)
            .unwrap();
        let native::ExactOperationWitness::Receive {
            step: b,
            incoming_payment,
        } = &signed_witness(&prepared, &released)
        else {
            panic!("Receive")
        };
        assert_eq!(incoming_payment.as_ref(), payment.exact);
        let pair = &payer.delivery.receiver_evidence().pair;
        let derive = |p: &CompilerPayment, b: &crate::receive::Witness| {
            status_from_selected(&payer, params, p, b, pair, gens, pins(full::RECEIVE))
        };
        let received = derive(&payment, &b).unwrap().require_decided().unwrap();
        assert_eq!(received.status, payer.delivery.wire_status().unwrap());
        assert_eq!(
            received.status_digest,
            payer.delivery.status_digest().unwrap()
        );
        assert_eq!(
            received.exact_pair(),
            payer.delivery.receiver_evidence().exact_pair
        );
        assert_eq!(
            received.credited,
            credited_digest(
                original.statement()[17],
                payment.digest,
                received.status_digest
            )
            .unwrap()
        );
        let mut changed = payment.clone();
        changed.exact[0] ^= 1;
        assert!(derive(&changed, &b).is_err());
        let mut changed = payment.clone();
        changed.pair_bytes[0] ^= 1;
        assert!(derive(&changed, &b).is_err());
        let mut changed = (*b).clone();
        changed.maps.credit_insert.slot.siblings[31] += Fp::ONE;
        assert!(derive(&payment, &changed).is_err());
        let mut changed = (*b).clone();
        changed.incoming_receipt.signature[7] ^= 1;
        assert!(derive(&payment, &changed).is_err());
    });
}
