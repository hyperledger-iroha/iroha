//! Canonically omit the all-short bitmap and the own IPA generator endpoints. Each
//! endpoint follows from its proof's L/R challenges before its Schnorr response.
use super::*;
use crate::{fs::Scalar, ipa};
use group::{Curve, Group, GroupEncoding};
use iroha_pasta::{
    Ep, Eq, PastaCurve,
    msm::{MemoryBudget, msm_public},
};

pub(super) struct Layout {
    bitmap: usize,
    primary: usize,
    secondary: usize,
    total: usize,
}

impl Layout {
    pub(super) fn new(
        profile: &carrier::TrustedProfile,
        codec: &proof_codec::Codec,
    ) -> Result<Self> {
        let ranges = carrier::projected_ranges(profile).map_err(Error::Rejected)?;
        let bitmap = codec.exception_bitmap_bytes();
        let primary = codec
            .encoded_word_offset(
                ranges
                    .primary
                    .end
                    .checked_sub(128)
                    .ok_or(Error::Malformed("later primary IPA tail"))?,
            )
            .map_err(Error::Rejected)?;
        let secondary = codec
            .encoded_word_offset(
                ranges
                    .secondary
                    .end
                    .checked_sub(128)
                    .ok_or(Error::Malformed("later secondary IPA tail"))?,
            )
            .map_err(Error::Rejected)?;
        let total = codec.encoded_len().map_err(Error::Rejected)?;
        if primary < bitmap || primary + 32 > secondary || secondary + 32 > total {
            return Err(Error::Malformed("later head generator layout"));
        }
        Ok(Self {
            bitmap,
            primary,
            secondary,
            total,
        })
    }

    pub(super) fn bytes(&self) -> usize {
        self.total - self.bitmap - 64
    }

    pub(super) fn compact(
        &self,
        codec: &proof_codec::Codec,
        profile: &carrier::TrustedProfile,
        raw: &[u8],
    ) -> Result<Vec<u8>> {
        let encoded = codec.pack(raw).map_err(Error::Rejected)?;
        let frame = carrier::frame_projected_pair(profile, raw).map_err(Error::Rejected)?;
        let own_point = |proof: &[u8]| -> Result<[u8; 32]> {
            let at = proof
                .len()
                .checked_sub(128)
                .ok_or(Error::Malformed("later head IPA tail"))?;
            Ok(proof[at..at + 32].try_into().unwrap())
        };
        if encoded.len() != self.total
            || encoded[..self.bitmap].iter().any(|b| *b != 0)
            || encoded[self.primary..self.primary + 32] != own_point(frame.primary)?
            || encoded[self.secondary..self.secondary + 32] != own_point(frame.secondary)?
        {
            return Err(Error::Malformed("later head exact generator offsets"));
        }
        Ok([
            &encoded[self.bitmap..self.primary],
            &encoded[self.primary + 32..self.secondary],
            &encoded[self.secondary + 32..],
        ]
        .concat())
    }

    pub(super) fn restore(
        &self,
        bytes: &[u8],
        codec: &proof_codec::Codec,
        profile: &carrier::TrustedProfile,
        projection: &carrier::ProjectedInputs,
        params: &AuthenticatedParameters,
    ) -> Result<Vec<u8>> {
        if bytes.len() != self.total - self.bitmap - 64 {
            return Err(Error::Malformed("later head fixed pair length"));
        }
        let mut packed = vec![0; self.total];
        let mut input = bytes;
        for range in [
            self.bitmap..self.primary,
            self.primary + 32..self.secondary,
            self.secondary + 32..self.total,
        ] {
            let len = range.len();
            packed[range].copy_from_slice(take(&mut input, len)?);
        }
        // Canonical stand-ins allow parsing the portion of the transcript preceding
        // ipa/G. They grant no authority. The ordinary native verifier then checks
        // both fully reconstructed proofs, their link, and all inherited claims.
        packed[self.primary..self.primary + 32]
            .copy_from_slice(&Eq::generator().to_affine().to_bytes());
        packed[self.secondary..self.secondary + 32]
            .copy_from_slice(&Ep::generator().to_affine().to_bytes());
        let mut raw = codec
            .unpack(&packed, &params.inner.generators)
            .map_err(Error::Rejected)?;
        let partial =
            carrier::decode_projected_pair(profile, &raw, projection).map_err(Error::Rejected)?;
        let u = assemble_pair(&params.inner, full::SEND, partial)?;
        let gens = &params.inner.generators;
        let traced = full::trace_incoming_candidate(u, gens, Vec::new());
        let pg = endpoint(&traced.tr_p.us, &gens.p)?;
        let sg = endpoint(&traced.tr_s.us, &gens.s)?;
        let frame = carrier::frame_projected_pair(profile, &raw).map_err(Error::Rejected)?;
        let p = frame.ranges.primary.end - 128;
        let s = frame.ranges.secondary.end - 128;
        raw[p..p + 32].copy_from_slice(&pg);
        raw[s..s + 32].copy_from_slice(&sg);
        Ok(raw)
    }
}

fn endpoint<C: PastaCurve>(challenges: &[C::ScalarExt], gens: &ipa::Gens<C>) -> Result<[u8; 32]>
where
    C::ScalarExt: Scalar,
{
    let n = 1usize
        .checked_shl(challenges.len() as u32)
        .ok_or(Error::ResourceLimit)?;
    let basis = gens
        .g
        .get(..n)
        .ok_or(Error::Malformed("later head generator capacity"))?;
    let point = msm_public::<C>(&ipa::s_vector(challenges), basis, MemoryBudget::DEFAULT)
        .map_err(|_| Error::Malformed("later head generator reconstruction"))?;
    Ok(point.to_affine().to_bytes())
}
