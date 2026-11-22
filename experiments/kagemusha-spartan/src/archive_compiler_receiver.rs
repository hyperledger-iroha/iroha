//! Receiver evidence for an explicitly selected compiler generation. The historical
//! Payment and hardware receipt stay unchanged when the same receiver head is folded
//! under a freshly generated Ω. Candidate tooling supplies independently selected
//! program digests and generators for both curves.
use super::*;
use crate::{carrier, native::engine::transfers::archive_sent::credited_digest, spartan::Link};
use iroha_data_model::kagemusha::KagemushaWalletCreditStatusV1;

pub(super) struct ScopedStatus {
    pub(super) status: KagemushaWalletCreditStatusV1,
    pub(super) receipt: ReceiverReceipt,
    pub(super) status_digest: [u8; 32],
    pub(super) credited: Fp,
    pub(super) payment: Fp,
}

impl ScopedStatus {
    pub(super) fn exact_pair(&self) -> &[u8] {
        &self.status.lineage.proof
    }
}

pub(super) fn reconstruct(
    payer: &Inputs<'_>,
    params: &native::AuthenticatedParameters,
    pair: &full::UniformPair,
    gens: &full::PairGens,
    expected_programs: [[u8; 32]; 2],
) -> native::Result<ScopedStatus> {
    if gens.defer_p.kinds.len() != 5 || gens.defer_s.kinds.len() != 5 {
        return Err(Error::AuthorityMismatch("five-kind receiver generation"));
    }
    from_selected_pair(payer, params, pair, gens, expected_programs)
}

fn from_selected_pair(
    payer: &Inputs<'_>,
    params: &native::AuthenticatedParameters,
    pair: &full::UniformPair,
    gens: &full::PairGens,
    expected_programs: [[u8; 32]; 2],
) -> native::Result<ScopedStatus> {
    let delivery = payer.delivery;
    let original = delivery.payment();
    let evidence = delivery.receiver_evidence();
    original.require_parameters(params)?;
    if receiver_kind(evidence) != pair.kind {
        return Err(Error::AuthorityMismatch(
            "compiler receiver scope or signed head kind",
        ));
    }
    let projection = native::engine::projection(&evidence.profile, pair.kind)?;
    let raw =
        checked_pair(params, pair, gens, expected_programs, &projection)?.require_decided()?;
    let mut status = delivery.wire_status()?;
    let credit = original.statement()[17];
    let payment = original.digest();
    if status.opening.credit_id != credit.to_repr()
        || status.opening.payment_digest != payment.to_repr()
    {
        return Err(Error::AuthorityMismatch(
            "compiler receiver retained Payment",
        ));
    }
    // The signed receipt's proof_digest authenticates its historical step; Ω is
    // the re-folded receiver head. The exact new Ω bytes enter the status digest.
    status.lineage.proof = raw;
    let status_digest = status
        .credit_status_digest()
        .map_err(|_| Error::AuthorityMismatch("compiler receiver CreditStatus"))?;
    let credited = credited_digest(credit, payment, status_digest)?;
    Ok(ScopedStatus {
        status,
        receipt: delivered_receipt(delivery)?,
        status_digest,
        credited,
        payment,
    })
}

/// The compiler can size an honestly derived Burn when a prior matrix polynomial
/// differs. Each flag is computed on the exact pair and current generators.
pub(super) struct Candidate<T> {
    pub(super) original: T,
    /// Primary matrix, secondary matrix, primary inherited generator, secondary inherited generator.
    pub(super) decisions: [bool; 4],
}

impl<T> Candidate<T> {
    pub(super) fn require_decided(self) -> native::Result<T> {
        if self.decisions != [true; 4] {
            return Err(Error::AuthorityMismatch(
                "compiler pair accumulator decision",
            ));
        }
        Ok(self.original)
    }
}

fn checked_pair(
    params: &native::AuthenticatedParameters,
    pair: &full::UniformPair,
    gens: &full::PairGens,
    expected_programs: [[u8; 32]; 2],
    projection: &carrier::ProjectedInputs,
) -> native::Result<Candidate<Vec<u8>>> {
    if [pair.cp.program_digest(), pair.cs.program_digest()] != expected_programs {
        return Err(Error::AuthorityMismatch(
            "compiler receiver program originals",
        ));
    }
    let trusted = carrier::TrustedProfile::from_parameters(
        params.inner.relation,
        pair.cp.shape,
        pair.cp.as_ref(),
        pair.cs.shape,
        pair.cs.as_ref(),
        true,
    )
    .map_err(Error::Rejected)?;
    let raw = carrier::encode_projected_pair(
        &trusted, &pair.ip, &pair.is, &pair.pp, &pair.ps, pair.r, projection,
    )
    .map_err(Error::Rejected)?;
    let decoded =
        carrier::decode_projected_pair(&trusted, &raw, projection).map_err(Error::Rejected)?;
    if (decoded.d, decoded.rho, decoded.r) != (pair.d, pair.rho, pair.r) {
        return Err(Error::AuthorityMismatch(
            "compiler receiver compact transcript",
        ));
    }
    let primary = crate::spartan::verify_linked_circuit(
        &pair.cp.shape,
        &gens.p,
        projection.primary(),
        &decoded.pp.to_bytes(),
        &Link::First {
            rho: decoded.rho,
            r: decoded.r,
        },
        pair.cp.as_ref(),
    )
    .map_err(|_| Error::Rejected("compiler receiver primary proof".into()))?;
    let fixed_r = |_: &[u8; 32]| decoded.r;
    let secondary = crate::spartan::verify_linked_circuit(
        &pair.cs.shape,
        &gens.s,
        projection.secondary(),
        &decoded.ps.to_bytes(),
        &Link::Second {
            d: decoded.d,
            rlc: &fixed_r,
        },
        pair.cs.as_ref(),
    )
    .map_err(|_| Error::Rejected("compiler receiver secondary proof".into()))?;
    if (primary, secondary) != (decoded.d, decoded.rho) {
        return Err(Error::AuthorityMismatch("compiler receiver linked proofs"));
    }
    let traced = full::trace_uniform(pair.clone(), gens);
    let inherited_p =
        full::inherited_generator_claim(&traced.tr_p, &gens.p).map_err(Error::Malformed)?;
    let inherited_s =
        full::inherited_generator_claim(&traced.tr_s, &gens.s).map_err(Error::Malformed)?;
    Ok(Candidate {
        original: raw,
        decisions: [
            native::engine::matrix_accumulator(&gens.defer_p, &pair.pp.late),
            native::engine::matrix_accumulator(&gens.defer_s, &pair.ps.late),
            crate::generator_claim::decide(&gens.p, &inherited_p),
            crate::generator_claim::decide(&gens.s, &inherited_s),
        ],
    })
}

#[path = "archive_compiler_new_payment.rs"]
mod fresh;
pub(super) use fresh::{
    CompilerPayment, new_payment, new_payment_candidate, new_receive_status,
    new_receive_status_candidate,
};

#[test]
#[ignore = "requires retained signed Receive and later Send originals"]
fn compiler_receiver_reconstructs_both_retained_heads_and_rejects_crossed_scope() {
    with_both_forms(|forms, _, _, params| {
        for (i, payer) in forms.iter().enumerate() {
            let delivery = payer.delivery;
            let evidence = delivery.receiver_evidence();
            let pair = &evidence.pair;
            let programs = &params.inner.programs[pair.kind];
            let pins = [
                programs.primary.program_digest(),
                programs.secondary.program_digest(),
            ];
            let gens = &params.inner.generators;
            let result = from_selected_pair(payer, params, pair, gens, pins).unwrap();
            assert_eq!(result.exact_pair(), evidence.exact_pair);
            assert_eq!(result.status, delivery.wire_status().unwrap());
            assert_eq!(result.status_digest, delivery.status_digest().unwrap());
            assert_eq!(result.payment, delivery.payment().digest());
            assert_eq!(result.receipt.send, receiver_kind(evidence) == full::SEND);
            assert_eq!(
                result.credited,
                credited_digest(
                    delivery.payment().statement()[17],
                    result.payment,
                    delivery.status_digest().unwrap()
                )
                .unwrap()
            );
            assert!(matches!(
                reconstruct(payer, params, pair, gens, pins),
                Err(Error::AuthorityMismatch("five-kind receiver generation"))
            ));
            let mut other = pins;
            other[0][0] ^= 1;
            assert!(from_selected_pair(payer, params, pair, gens, other).is_err());
            other = pins;
            other[1][0] ^= 1;
            assert!(from_selected_pair(payer, params, pair, gens, other).is_err());
            assert!(from_selected_pair(forms[1 - i], params, pair, gens, pins).is_err());
            let mut wrong_head = pair.clone();
            wrong_head.ip.io[full::HEAD_IO] += Fp::ONE;
            assert!(from_selected_pair(payer, params, &wrong_head, gens, pins).is_err());
            let mut wrong_projection = pair.clone();
            wrong_projection.is.io[1] += Fq::ONE;
            assert!(from_selected_pair(payer, params, &wrong_projection, gens, pins).is_err());
            let mut wrong_link = pair.clone();
            wrong_link.rho[0] ^= 1;
            assert!(from_selected_pair(payer, params, &wrong_link, gens, pins).is_err());
        }
    });
}
