//! Reversible representations of public Payment words. An exception bit carries
//! the exact original 32 bytes when the resulting message fits its transport limits.
use ff::{PrimeField, WithSmallOrderMulGroup};
use iroha_pasta::{Ep, Eq, PastaCurve};

#[derive(Clone, Copy)]
pub(crate) enum Encoding {
    Low(usize),
    PrimaryEndo,
    SecondaryEndo,
}

const BASE: u128 = 1 << 63;

fn endo<C: PastaCurve>(word: [u8; 32]) -> Option<[u8; 16]> {
    let scalar = Option::from(C::ScalarExt::from_repr(word))?;
    let split = C::glv_decompose(&scalar)?;
    if split.k1_neg || split.k2_neg {
        return None;
    }
    let offset = |n: u128| u64::try_from(n.checked_sub(BASE)?).ok().filter(|&v| v != 0);
    let mut out = [0; 16];
    // glv gives k1 + k2 * ZETA. The transcript uses a * ZETA + b.
    out[..8].copy_from_slice(&offset(split.k2)?.to_le_bytes());
    out[8..].copy_from_slice(&offset(split.k1)?.to_le_bytes());
    (expand_endo::<C>(&out).ok()? == word).then_some(out)
}

fn expand_endo<C: PastaCurve>(value: &[u8]) -> Result<[u8; 32], String> {
    let coeff = |bytes: &[u8]| -> Result<C::ScalarExt, String> {
        let offset = u64::from_le_bytes(bytes.try_into().map_err(|_| "endo coefficient width")?);
        if offset == 0 {
            return Err("endo coefficient outside the 126-bit challenge range".into());
        }
        Ok(C::ScalarExt::from_u128(BASE + u128::from(offset)))
    };
    Ok((coeff(&value[..8])? * C::ScalarExt::ZETA + coeff(&value[8..])?).to_repr())
}

impl Encoding {
    fn width(self) -> usize {
        match self {
            Self::Low(width) => width,
            Self::PrimaryEndo | Self::SecondaryEndo => 16,
        }
    }
    fn shorten(self, word: [u8; 32]) -> Option<Vec<u8>> {
        match self {
            Self::Low(width) => word[width..]
                .iter()
                .all(|&v| v == 0)
                .then(|| word[..width].to_vec()),
            Self::PrimaryEndo => endo::<Eq>(word).map(|v| v.to_vec()),
            Self::SecondaryEndo => endo::<Ep>(word).map(|v| v.to_vec()),
        }
    }
    fn expand(self, bytes: &[u8]) -> Result<[u8; 32], String> {
        match self {
            Self::Low(width) => {
                let mut word = [0; 32];
                word[..width].copy_from_slice(bytes);
                Ok(word)
            }
            Self::PrimaryEndo => expand_endo::<Eq>(bytes),
            Self::SecondaryEndo => expand_endo::<Ep>(bytes),
        }
    }
}

pub(crate) type Positions = Vec<(usize, Encoding)>;

pub(crate) fn all_short_size(
    total: usize,
    positions: &[(usize, Encoding)],
) -> Result<usize, String> {
    let mut end = 0;
    let mut saved = 0;
    for &(at, encoding) in positions {
        if at < end || at.checked_add(32).is_none_or(|n| n > total) || encoding.width() > 32 {
            return Err("public Payment word range".into());
        }
        end = at + 32;
        saved += 32 - encoding.width();
    }
    (total - saved)
        .checked_add(positions.len().div_ceil(8))
        .ok_or("Payment length overflow".into())
}

pub(crate) fn pack(original: &[u8], positions: &[(usize, Encoding)]) -> Result<Vec<u8>, String> {
    let mut out = vec![0; positions.len().div_ceil(8)];
    let mut offset = 0;
    for (i, &(at, encoding)) in positions.iter().enumerate() {
        out.extend_from_slice(
            original
                .get(offset..at)
                .ok_or("public Payment word order")?,
        );
        let word: [u8; 32] = original
            .get(at..at + 32)
            .ok_or("public Payment word range")?
            .try_into()
            .unwrap();
        if let Some(short) = encoding.shorten(word) {
            out.extend_from_slice(&short);
        } else {
            out[i / 8] |= 1 << (i % 8);
            out.extend_from_slice(&word);
        }
        offset = at + 32;
    }
    out.extend_from_slice(&original[offset..]);
    Ok(out)
}

pub(crate) fn unpack(
    encoded: &[u8],
    total: usize,
    positions: &[(usize, Encoding)],
) -> Result<Vec<u8>, String> {
    let bitmap_size = positions.len().div_ceil(8);
    let (bitmap, mut rest) = encoded
        .split_at_checked(bitmap_size)
        .ok_or("truncated Payment word bitmap")?;
    if !positions.len().is_multiple_of(8) && bitmap[bitmap_size - 1] >> (positions.len() % 8) != 0 {
        return Err("reserved Payment word bitmap bits".into());
    }
    let mut out = Vec::with_capacity(total);
    fn take<'a>(rest: &mut &'a [u8], n: usize) -> Result<&'a [u8], String> {
        let (v, next) = rest
            .split_at_checked(n)
            .ok_or("truncated public Payment words")?;
        *rest = next;
        Ok(v)
    }
    for (i, &(at, encoding)) in positions.iter().enumerate() {
        let gap = at
            .checked_sub(out.len())
            .ok_or("public Payment word order")?;
        if at.checked_add(32).is_none_or(|end| end > total) {
            return Err("public Payment word range".into());
        }
        out.extend_from_slice(take(&mut rest, gap)?);
        let word = if bitmap[i / 8] & (1 << (i % 8)) != 0 {
            let word: [u8; 32] = take(&mut rest, 32)?.try_into().unwrap();
            if encoding.shorten(word).is_some() {
                return Err("unnecessary raw Payment word exception".into());
            }
            word
        } else {
            let bytes = take(&mut rest, encoding.width())?;
            let word = encoding.expand(bytes)?;
            if encoding.shorten(word).as_deref() != Some(bytes) {
                return Err("noncanonical compact Payment word".into());
            }
            word
        };
        out.extend_from_slice(&word);
    }
    if rest.len() != total - out.len() {
        return Err("public Payment words length".into());
    }
    out.extend_from_slice(rest);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ff::Field;

    #[test]
    fn endoscalar_orientation_and_raw_exceptions_are_exact_on_both_curves() {
        fn check<C: PastaCurve>(encoding: Encoding) {
            // All 63 additions choose the same coordinate and sign. Its
            // biased value reaches one or u64::MAX on each curve.
            for sign in [false, true] {
                for first_coordinate in [false, true] {
                    let bits = [sign, first_coordinate].repeat(crate::fs::CHALLENGE_BITS / 2);
                    let word = crate::fs::endo_scalar::<C::ScalarExt>(&bits).to_repr();
                    let encoded = encoding.shorten(word).expect("bounded challenge");
                    let offset = if first_coordinate { 0 } else { 8 };
                    let endpoint = if sign { u64::MAX } else { 1 };
                    assert_eq!(&encoded[offset..offset + 8], &endpoint.to_le_bytes());
                    assert_eq!(encoding.expand(&encoded).unwrap(), word);
                }
            }
            for n in 0..128usize {
                let bits: Vec<_> = (0..126usize)
                    .map(|i| {
                        (i.wrapping_mul(0x459) ^ n.wrapping_mul(0x93) ^ (i >> (n % 6))) & 4 != 0
                    })
                    .collect();
                let (a, b, _, _) = crate::fs::endo_coefficients(&bits);
                let word = crate::fs::endo_scalar::<C::ScalarExt>(&bits).to_repr();
                let positions = [(0, encoding)];
                let short = pack(&word, &positions).unwrap();
                assert_eq!(short.len(), 17);
                assert_eq!(&short[1..9], &((a - BASE) as u64).to_le_bytes());
                assert_eq!(&short[9..17], &((b - BASE) as u64).to_le_bytes());
                assert_eq!(unpack(&short, 32, &positions).unwrap(), word);
                let mut alternative = vec![1];
                alternative.extend_from_slice(&word);
                assert!(unpack(&alternative, 32, &positions).is_err());
                let mut reserved = short;
                reserved[0] |= 0x80;
                assert!(unpack(&reserved, 32, &positions).is_err());
            }
            for word in [
                [0xff; 32],
                C::ScalarExt::ZERO.to_repr(),
                C::ScalarExt::ONE.to_repr(),
            ] {
                let positions = [(0, encoding)];
                let encoded = pack(&word, &positions).unwrap();
                assert_eq!(encoded[0], 1);
                assert_eq!(unpack(&encoded, 32, &positions).unwrap(), word);
            }
        }
        check::<Eq>(Encoding::PrimaryEndo);
        check::<Ep>(Encoding::SecondaryEndo);
    }
}
