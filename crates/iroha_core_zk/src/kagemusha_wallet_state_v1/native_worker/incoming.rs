//! Native witness proposals for the circuit's total incoming proof decoder.
//!
//! Original bytes remain unchanged for every digest. Invalid fixed-offset views
//! receive the exact circuit dummies; decoder failures and succinct-proof failures
//! remain false. Resource/profile failures are errors and cannot authorize a burn.

use ff::Field;
use iroha_kagemusha_proof::{
    a_relation::native::artifact::KeyArtifact,
    q_sigma::native::{IncomingMode, incoming_proof_failure as invalid_proof},
};
use iroha_pasta::{Ep, Eq, PastaAffine, PastaCurve, poseidon::hash_with_domain};
use iroha_plonk::{Protocol, pcs::ipa::PinnedParams, verifier::accumulate_generator};
use iroha_plonk_gadgets::statement::foreign_limbs;
use iroha_plonk_recursion::{ACCUMULATOR_BYTES, AccumulatorT, FoldInput};

use super::*;

pub(super) struct TransportV1 {
    pub(super) public: [Fp; 18],
    pub(super) public_valid: bool,
    pub(super) pallas: AccumulatorT<Ep>,
    pub(super) vesta: AccumulatorT<Eq>,
    pub(super) opening: FoldInput<Ep>,
    pub(super) valid: bool,
}

fn view<const N: usize>(original: &[u8], start: usize) -> [u8; N] {
    std::array::from_fn(|i| original.get(start + i).copied().unwrap_or(0))
}
fn u128_at(original: &[u8], start: usize) -> Fp {
    Fp::from_u128(u128::from_le_bytes(view(original, start)))
}

/// Exact incoming_lineage.rs offsets and zero-selection of noncanonical Fp atoms.
fn public(original: &[u8], expected: usize, key: Fp) -> ([Fp; 18], bool) {
    let mut out = [Fp::ZERO; 18];
    out[0] = Fp::from(u64::from(u16::from_le_bytes(view(original, 0))));
    let mut valid =
        original.len() == expected && out[0] == Fp::ONE && original.get(162) == Some(&4);
    for (offset, index) in [(2, 1), (34, 3), (98, 6)] {
        out[index] = u128_at(original, offset);
        out[index + 1] = u128_at(original, offset + 16);
    }
    for (offset, index) in [(66, 5), (130, 8), (256, 15), (288, 16)] {
        let decoded = Option::<Fp>::from(Fp::from_repr(view(original, offset)));
        valid &= decoded.is_some();
        out[index] = decoded.unwrap_or(Fp::ZERO);
    }
    for (limb, index) in [10, 9, 12, 11].into_iter().enumerate() {
        out[index] = Fp::from_u128(u128::from_be_bytes(view(original, 163 + 16 * limb)));
    }
    let lifecycle = u128::from(original.get(227).copied().unwrap_or(0));
    let epoch = u128::from(u64::from_le_bytes(view(original, 228)));
    let controls = u128::from(u32::from_le_bytes(view(original, 236)));
    out[13] = Fp::from_u128(lifecycle | epoch << 8 | controls << 72);
    out[14] = u128_at(original, 240);
    out[17] = key;
    (out, valid)
}

fn opening<C: PastaCurve>(
    params: &PinnedParams<C>,
    key: &KeyArtifact<C>,
    instances: &[Vec<C::ScalarExt>],
    bytes: &[u8],
    budget: MemoryBudget,
) -> Result<Option<FoldInput<C>>, Error> {
    match accumulate_generator(params, key.binding(), key.key(), instances, bytes, budget) {
        Ok(value) => Ok(Some(proof(FoldInput::from_opening(
            *value.g(),
            value.challenges(),
        ))?)),
        Err(error) if invalid_proof(&error) => Ok(None),
        Err(_) => Err(Error::Proof("incoming verifier resources or profile")),
    }
}

pub(super) fn transport(
    key: &KeyArtifact<Ep>,
    original: &[u8],
    budget: MemoryBudget,
) -> Result<TransportV1, Error> {
    let proof_bytes = proof(Protocol::new(key.binding().descriptor()))?.proof_length();
    let total = 320usize
        .checked_add(proof_bytes)
        .and_then(|n| n.checked_add(2 * ACCUMULATOR_BYTES))
        .ok_or(Error::Proof("incoming transport bounds"))?;
    let pparams = proof(PinnedParams::<Ep>::derive(16))?;
    let vparams = proof(PinnedParams::<Eq>::derive(16))?;
    let trivial_p = proof(AccumulatorT::trivial(&pparams, budget))?;
    let trivial_v = proof(AccumulatorT::trivial(&vparams, budget))?;
    let key_digest = proof(key.key().kagemusha_digest(key.binding()))?;
    let (public, public_valid) = public(original, total, key_digest);
    let pallas =
        AccumulatorT::<Ep>::from_bytes(&view::<ACCUMULATOR_BYTES>(original, 320 + proof_bytes));
    let vesta = AccumulatorT::<Eq>::from_bytes(&view::<ACCUMULATOR_BYTES>(
        original,
        320 + proof_bytes + ACCUMULATOR_BYTES,
    ));
    let claims_valid = pallas.is_ok() && vesta.is_ok();
    let pallas = pallas.unwrap_or_else(|_| trivial_p.clone());
    let vesta = vesta.unwrap_or(trivial_v);
    let mut words = public.to_vec();
    let (x, y) =
        Option::<(Fp, Fp)>::from(pallas.g().coordinates()).ok_or(Error::Proof("incoming point"))?;
    words.extend([x, y]);
    for challenge in pallas.challenges() {
        words.extend(foreign_limbs(challenge).map(Fp::from_u128));
    }
    let digest = hash_with_domain(u64::from_le_bytes(*b"kgwomg_1"), &words);
    let (x, y) =
        Option::<(Fq, Fq)>::from(vesta.g().coordinates()).ok_or(Error::Proof("incoming point"))?;
    let instances = vec![
        vec![
            Option::<Fq>::from(Fq::from_repr(digest.to_repr()))
                .ok_or(Error::Proof("incoming digest"))?,
        ],
        vec![x, y],
        vesta
            .challenges()
            .iter()
            .map(|v| {
                Option::<Fq>::from(Fq::from_repr(v.to_repr()))
                    .ok_or(Error::Proof("incoming challenge"))
            })
            .collect::<Result<Vec<_>, _>>()?,
    ];
    // The circuit verifies the fixed padded view even when the outer length is
    // wrong, then conjoins outer validity. Preserve that opening on every path.
    let bytes = (0..proof_bytes)
        .map(|i| original.get(320 + i).copied().unwrap_or(0))
        .collect::<Vec<_>>();
    let actual = opening(&pparams, key, &instances, &bytes, budget)?;
    let valid = public_valid && claims_valid && actual.is_some();
    Ok(TransportV1 {
        public,
        public_valid,
        pallas,
        vesta,
        opening: actual.unwrap_or_else(|| trivial_p.as_input()),
        valid,
    })
}

pub(super) fn sigma(
    key: &KeyArtifact<Eq>,
    statement: Fp,
    original: &[u8],
    budget: MemoryBudget,
) -> Result<Option<FoldInput<Eq>>, Error> {
    let expected = proof(Protocol::new(key.binding().descriptor()))?.proof_length();
    if original.len() != expected {
        return Ok(None);
    }
    let params = proof(PinnedParams::<Eq>::derive(u32::from(
        key.binding().descriptor().k,
    )))?;
    opening(&params, key, &[vec![statement]], original, budget)
}

/// Select exactly one correction if all succinct predicates pass but a deferred
/// obligation fails. Any false predicate takes the all-Trivial branch instead.
pub(super) fn modes(
    soft: bool,
    transport: &TransportV1,
    sigma: Option<&FoldInput<Eq>>,
    budget: MemoryBudget,
) -> Result<
    (
        [IncomingMode; 4],
        [iroha_pasta::EpAffine; 2],
        iroha_pasta::EqAffine,
    ),
    Error,
> {
    let (transport_modes, pc, vc) = status_modes(soft, transport, budget)?;
    let mut selected = [IncomingMode::Trivial; 4];
    selected[..3].copy_from_slice(&transport_modes);
    if !soft || transport_modes.contains(&IncomingMode::Corrected) {
        return Ok((selected, pc, vc));
    }
    selected[3] = sigma_mode(true, sigma, budget)?;
    if selected[3] == IncomingMode::Corrected {
        selected[..3].fill(IncomingMode::Trivial);
    }
    Ok((selected, pc, vc))
}

/// Archive Status has exactly the three original Omega obligations and no sigma.
pub(super) fn status_modes(
    soft: bool,
    transport: &TransportV1,
    budget: MemoryBudget,
) -> Result<
    (
        [IncomingMode; 3],
        [iroha_pasta::EpAffine; 2],
        iroha_pasta::EqAffine,
    ),
    Error,
> {
    let p = proof(PinnedParams::<Ep>::derive(16))?;
    let v = proof(PinnedParams::<Eq>::derive(16))?;
    let trivial_p = proof(AccumulatorT::trivial(&p, budget))?;
    let trivial_v = proof(AccumulatorT::trivial(&v, budget))?;
    let mut corrections = [*trivial_p.g(); 2];
    let mut vcorrection = *trivial_v.g();
    if !soft {
        return Ok(([IncomingMode::Trivial; 3], corrections, vcorrection));
    }
    let mut selected = [IncomingMode::Accept; 3];
    for (index, claim) in [transport.pallas.as_input(), transport.opening.clone()]
        .iter()
        .enumerate()
    {
        match claim.decide(&p, budget) {
            Ok(()) => {}
            Err(iroha_plonk_recursion::Error::Undecidable) => {
                corrections[index] = *proof(claim.corrected(&p, budget))?.replacement().g();
                selected.fill(IncomingMode::Trivial);
                selected[index] = IncomingMode::Corrected;
                return Ok((selected, corrections, vcorrection));
            }
            Err(_) => return Err(Error::Proof("incoming decide resources")),
        }
    }
    match transport.vesta.as_input().decide(&v, budget) {
        Ok(()) => {}
        Err(iroha_plonk_recursion::Error::Undecidable) => {
            vcorrection = *proof(transport.vesta.as_input().corrected(&v, budget))?
                .replacement()
                .g();
            selected.fill(IncomingMode::Trivial);
            selected[2] = IncomingMode::Corrected;
        }
        Err(_) => return Err(Error::Proof("incoming decide resources")),
    }
    Ok((selected, corrections, vcorrection))
}

/// Archive Receive has one sigma obligation and no transported Omega claims.
/// The actual Q_sigma producer derives and proves its same-challenge correction.
pub(super) fn sigma_mode(
    soft: bool,
    sigma: Option<&FoldInput<Eq>>,
    budget: MemoryBudget,
) -> Result<IncomingMode, Error> {
    if !soft {
        return Ok(IncomingMode::Trivial);
    }
    let sigma = sigma.ok_or(Error::Proof("incoming sigma obligation"))?;
    let params = proof(PinnedParams::<Eq>::derive(16))?;
    match sigma.decide(&params, budget) {
        Ok(()) => Ok(IncomingMode::Accept),
        Err(iroha_plonk_recursion::Error::Undecidable) => Ok(IncomingMode::Corrected),
        Err(_) => Err(Error::Proof("incoming decide resources")),
    }
}

#[cfg(test)]
mod tests {
    use iroha_plonk::{
        VerifyError,
        pcs::{ipa::IpaError, multiopen::MultiopenError},
        transcript::TranscriptError,
    };

    use super::*;
    #[test]
    fn total_failure_classification_keeps_nested_proof_errors_soft_and_profiles_hard() {
        for error in [
            VerifyError::Transcript(TranscriptError::ProofTruncated),
            VerifyError::Ipa(IpaError::Transcript(TranscriptError::ProofTruncated)),
            VerifyError::Multiopen(MultiopenError::Ipa(IpaError::Transcript(
                TranscriptError::ProofTruncated,
            ))),
            VerifyError::Ipa(IpaError::OpeningFailed),
            VerifyError::IdentityInstanceCommitment { column: 0 },
            VerifyError::ProofLength {
                expected: 32,
                actual: 31,
            },
        ] {
            assert!(invalid_proof(&error), "{error:?}");
        }
        for error in [
            VerifyError::KeyMismatch,
            VerifyError::ParamsMismatch,
            VerifyError::SuffixRequired,
            VerifyError::Transcript(TranscriptError::ProfileMismatch),
            VerifyError::Ipa(IpaError::Transcript(TranscriptError::ProfileMismatch)),
            VerifyError::Ipa(IpaError::ParamsTooSmall {
                needed: 16,
                available: 12,
            }),
        ] {
            assert!(!invalid_proof(&error), "{error:?}");
        }
    }

    #[test]
    fn incoming_mode_selection_preserves_all_four_obligations_and_one_correction() {
        use p256::elliptic_curve::group::prime::PrimeCurveAffine;
        let budget = MemoryBudget::DEFAULT;
        let pp = PinnedParams::<Ep>::derive(16).unwrap();
        let vp = PinnedParams::<Eq>::derive(16).unwrap();
        let p = AccumulatorT::trivial(&pp, budget).unwrap();
        let v = AccumulatorT::trivial(&vp, budget).unwrap();
        let mut input = TransportV1 {
            public: [Fp::ZERO; 18],
            public_valid: true,
            pallas: p.clone(),
            vesta: v.clone(),
            opening: p.as_input(),
            valid: true,
        };
        let sigma = v.as_input();
        assert_eq!(
            modes(true, &input, Some(&sigma), budget).unwrap().0,
            [IncomingMode::Accept; 4]
        );
        assert_eq!(
            modes(false, &input, None, budget).unwrap().0,
            [IncomingMode::Trivial; 4]
        );
        let bad_p =
            AccumulatorT::<Ep>::new(iroha_pasta::EpAffine::generator(), *p.challenges()).unwrap();
        let bad_v =
            AccumulatorT::<Eq>::new(iroha_pasta::EqAffine::generator(), *v.challenges()).unwrap();
        assert!(matches!(
            bad_p.decide(&pp, budget),
            Err(iroha_plonk_recursion::Error::Undecidable)
        ));
        assert!(matches!(
            bad_v.decide(&vp, budget),
            Err(iroha_plonk_recursion::Error::Undecidable)
        ));
        for selected in 0..4 {
            input.pallas = if selected == 0 {
                bad_p.clone()
            } else {
                p.clone()
            };
            input.opening = if selected == 1 {
                bad_p.as_input()
            } else {
                p.as_input()
            };
            input.vesta = if selected == 2 {
                bad_v.clone()
            } else {
                v.clone()
            };
            let sigma = if selected == 3 {
                bad_v.as_input()
            } else {
                v.as_input()
            };
            let (actual, pc, vc) = modes(true, &input, Some(&sigma), budget).unwrap();
            let mut expected = [IncomingMode::Trivial; 4];
            expected[selected] = IncomingMode::Corrected;
            assert_eq!(actual, expected);
            if selected < 2 {
                assert_ne!(pc[selected], *bad_p.g());
            }
            if selected == 2 {
                assert_ne!(vc, *bad_v.g());
            }
        }
        // A failed soft predicate takes precedence over every nondeciding original.
        input.pallas = bad_p;
        input.vesta = bad_v;
        assert_eq!(
            modes(false, &input, None, budget).unwrap().0,
            [IncomingMode::Trivial; 4]
        );
    }

    #[test]
    fn archive_variants_select_only_their_actual_obligations() {
        use p256::elliptic_curve::group::prime::PrimeCurveAffine;
        let budget = MemoryBudget::DEFAULT;
        let pp = PinnedParams::<Ep>::derive(16).unwrap();
        let vp = PinnedParams::<Eq>::derive(16).unwrap();
        let p = AccumulatorT::trivial(&pp, budget).unwrap();
        let v = AccumulatorT::trivial(&vp, budget).unwrap();
        let mut input = TransportV1 {
            public: [Fp::ZERO; 18],
            public_valid: true,
            pallas: p.clone(),
            vesta: v.clone(),
            opening: p.as_input(),
            valid: true,
        };
        assert_eq!(
            status_modes(true, &input, budget).unwrap().0,
            [IncomingMode::Accept; 3]
        );
        assert_eq!(
            status_modes(false, &input, budget).unwrap().0,
            [IncomingMode::Trivial; 3]
        );
        for k in [12, 14] {
            let bad =
                FoldInput::from_opening(iroha_pasta::EqAffine::generator(), &vec![Fp::ONE; k])
                    .unwrap();
            let correction = bad.corrected(&vp, budget).unwrap();
            let good = correction.replacement();
            assert_eq!(
                sigma_mode(true, Some(good), budget).unwrap(),
                IncomingMode::Accept
            );
            assert_eq!(
                sigma_mode(true, Some(&bad), budget).unwrap(),
                IncomingMode::Corrected
            );
        }
        assert!(sigma_mode(true, None, budget).is_err());
        assert_eq!(
            sigma_mode(false, None, budget).unwrap(),
            IncomingMode::Trivial
        );
        input.vesta =
            AccumulatorT::new(iroha_pasta::EqAffine::generator(), *v.challenges()).unwrap();
        assert_eq!(
            status_modes(true, &input, budget).unwrap().0,
            [
                IncomingMode::Trivial,
                IncomingMode::Trivial,
                IncomingMode::Corrected
            ]
        );
    }

    #[test]
    fn total_prefix_preserves_raw_fields_and_zero_selects_only_noncanonical_atoms() {
        let mut raw = vec![0; 4800];
        raw[0] = 1;
        raw[162] = 4;
        for offset in [66, 130, 256, 288] {
            raw[offset] = 1;
        }
        let (fields, valid) = public(&raw, 4800, Fp::from(7));
        assert!(valid);
        assert_eq!(fields[17], Fp::from(7));
        raw[66..98].fill(255);
        let (fields, valid) = public(&raw, 4800, Fp::from(7));
        assert!(!valid);
        assert_eq!(fields[5], Fp::ZERO);
        assert_eq!(fields[8], Fp::ONE);
        raw[66..98].fill(0);
        raw[66] = 1;
        raw[162] = 3;
        assert!(!public(&raw, 4800, Fp::from(7)).1);
        raw[162] = 4;
        raw.push(0);
        assert!(!public(&raw, 4800, Fp::from(7)).1);
        assert!(!public(&[], 4800, Fp::from(7)).1);
    }
}
