#![allow(non_snake_case)]
#![allow(non_upper_case_globals)]

use super::super::comm::PRNG;

// ========================================================================
// Gaussian sampling for (f,g)
// ========================================================================

// This code samples the secret polynomials f and g deterministically
// from a given seed. The polynomial coefficients follow a given
// Gaussian distribution centred on zero. A PRNG (type parameter) is used
// to produce random 16-bit samples which are then used in a CDT table.

const GTAB_8: [u16; 48] = [
        1,     3,     6,    11,    22,    40,    73,   129,
      222,   371,   602,   950,  1460,  2183,  3179,  4509,
     6231,  8395, 11032, 14150, 17726, 21703, 25995, 30487,
    35048, 39540, 43832, 47809, 51385, 54503, 57140, 59304,
    61026, 62356, 63352, 64075, 64585, 64933, 65164, 65313,
    65406, 65462, 65495, 65513, 65524, 65529, 65532, 65534,
];

const GTAB_9: [u16; 34] = [
        1,     4,    11,    28,    65,   146,   308,   615,
     1164,  2083,  3535,  5692,  8706, 12669, 17574, 23285,
    29542, 35993, 42250, 47961, 52866, 56829, 59843, 62000,
    63452, 64371, 64920, 65227, 65389, 65470, 65507, 65524,
    65531, 65534,
];

const GTAB_10: [u16; 24] = [
        2,     8,    28,    94,   280,   742,  1761,  3753,
     7197, 12472, 19623, 28206, 37329, 45912, 53063, 58338,
    61782, 63774, 64793, 65255, 65441, 65507, 65527, 65533
];

// Sample the f (or g) polynomial, using the provided PRNG,
// for a given degree n = 2^logn (with 1 <= logn <= 10). This function
// ensures that the returned polynomial has odd parity.
pub(crate) fn sample_f_bounded<T: PRNG>(
    logn: u32,
    rng: &mut T,
    f: &mut [i8],
    max_parity_attempts: u32,
) -> bool {
    assert!(1 <= logn && logn <= 10);
    let n = 1 << logn;
    assert!(f.len() == n);
    let (tab, zz) = match logn {
        9 => (&GTAB_9[..], 1),
        10 => (&GTAB_10[..], 1),
        _ => (&GTAB_8[..], 1 << (8 - logn)),
    };
    let kmax = (tab.len() >> 1) as i32;

    for _ in 0..max_parity_attempts {
        let mut parity = 0;
        let mut i = 0;
        while i < n {
            let mut v = 0;
            for _ in 0..zz {
                let y = rng.next_u16() as u32;
                v -= kmax;
                for k in 0..tab.len() {
                    v += (((tab[k] as u32).wrapping_sub(y)) >> 31) as i32;
                }
            }
            // For reduced/test degrees 2^6 or less, the value may be outside
            // of [-127, +127], which we do not want. This cannot happen for
            // degrees 2^7 and more, in particular for the "normal" degrees
            // 512 and 1024.
            if v < -127 || v > 127 {
                continue;
            }
            f[i] = v as i8;
            i += 1;
            parity ^= v as u32;
        }

        // We need an odd parity (so that the resultant of f with X^n+1 is
        // an odd integer).
        if (parity & 1) != 0 {
            return true;
        }
    }
    false
}
