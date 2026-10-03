//! Reed–Solomon over GF(2^8) with errors-and-erasures decoding.
//!
//! The field uses the primitive polynomial `x^8 + x^4 + x^3 + x^2 + 1`
//! (`0x11D`, the same field as QR Code) with `α = 2`. A codeword is
//! `data || parity`, systematic, and the generator is
//! `g(x) = (x - α^0)(x - α^1)…(x - α^(nsym-1))`, so the first consecutive root
//! is `α^0`. The first byte of a codeword is the highest-degree coefficient.
//!
//! Every Petal lane is one codeword (`n <= 255`). Low-confidence cells are
//! passed to [`ReedSolomon::decode`] as erasures, which cost one parity symbol
//! each instead of two.

/// Primitive polynomial of the field.
const PRIMITIVE: u16 = 0x11D;

const fn make_tables() -> ([u8; 512], [u8; 256]) {
    let mut exp = [0u8; 512];
    let mut log = [0u8; 256];
    let mut x: u16 = 1;
    let mut i = 0;
    while i < 255 {
        exp[i] = x as u8;
        log[x as usize] = i as u8;
        x <<= 1;
        if x & 0x100 != 0 {
            x ^= PRIMITIVE;
        }
        i += 1;
    }
    let mut j = 255;
    while j < 512 {
        exp[j] = exp[j - 255];
        j += 1;
    }
    (exp, log)
}

static TABLES: ([u8; 512], [u8; 256]) = make_tables();

/// Multiplies two field elements.
#[must_use]
pub fn gf_mul(a: u8, b: u8) -> u8 {
    if a == 0 || b == 0 {
        0
    } else {
        TABLES.0[usize::from(TABLES.1[usize::from(a)]) + usize::from(TABLES.1[usize::from(b)])]
    }
}

/// Divides `a` by a non-zero `b`.
fn gf_div(a: u8, b: u8) -> u8 {
    debug_assert!(b != 0, "division by zero in GF(256)");
    if a == 0 {
        0
    } else {
        TABLES.0
            [usize::from(TABLES.1[usize::from(a)]) + 255 - usize::from(TABLES.1[usize::from(b)])]
    }
}

/// Returns `α^exponent`.
#[must_use]
pub fn gf_exp(exponent: usize) -> u8 {
    TABLES.0[exponent % 255]
}

/// Returns the multiplicative inverse of a non-zero element.
fn gf_inv(a: u8) -> u8 {
    debug_assert!(a != 0, "inverse of zero in GF(256)");
    TABLES.0[255 - usize::from(TABLES.1[usize::from(a)])]
}

/// Multiplies two polynomials stored lowest-degree first.
fn poly_mul(a: &[u8], b: &[u8]) -> Vec<u8> {
    let mut out = vec![0u8; a.len() + b.len() - 1];
    for (i, &x) in a.iter().enumerate() {
        if x == 0 {
            continue;
        }
        for (j, &y) in b.iter().enumerate() {
            out[i + j] ^= gf_mul(x, y);
        }
    }
    out
}

/// Evaluates a lowest-degree-first polynomial at `x` (Horner).
fn poly_eval(poly: &[u8], x: u8) -> u8 {
    let mut acc = 0u8;
    for &coefficient in poly.iter().rev() {
        acc = gf_mul(acc, x) ^ coefficient;
    }
    acc
}

/// Reasons a Reed–Solomon decode can fail.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RsError {
    /// The codeword length, parity count or erasure list is invalid.
    InvalidShape,
    /// More errata than the code can correct, or the word is not decodable.
    Uncorrectable,
}

impl core::fmt::Display for RsError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InvalidShape => f.write_str("invalid Reed-Solomon codeword shape"),
            Self::Uncorrectable => f.write_str("Reed-Solomon word is uncorrectable"),
        }
    }
}

impl std::error::Error for RsError {}

/// A Reed–Solomon code with a fixed number of parity bytes.
#[derive(Debug, Clone)]
pub struct ReedSolomon {
    nsym: usize,
    generator: Vec<u8>,
}

impl ReedSolomon {
    /// Creates a code with `nsym` parity bytes.
    ///
    /// # Panics
    /// Panics when `nsym` is zero or exceeds 254.
    #[must_use]
    pub fn new(nsym: usize) -> Self {
        assert!((1..=254).contains(&nsym), "parity byte count out of range");
        // Highest-degree-first monic generator.
        let mut generator = vec![1u8];
        for i in 0..nsym {
            let root = gf_exp(i);
            let mut next = vec![0u8; generator.len() + 1];
            for (k, &coefficient) in generator.iter().enumerate() {
                next[k] ^= coefficient;
                next[k + 1] ^= gf_mul(coefficient, root);
            }
            generator = next;
        }
        Self { nsym, generator }
    }

    /// Number of parity bytes.
    #[must_use]
    pub fn parity_len(&self) -> usize {
        self.nsym
    }

    /// Encodes `data`, returning `data || parity`.
    ///
    /// # Panics
    /// Panics when the codeword would exceed 255 bytes.
    #[must_use]
    pub fn encode(&self, data: &[u8]) -> Vec<u8> {
        assert!(
            data.len() + self.nsym <= 255,
            "Reed-Solomon codeword longer than 255 bytes"
        );
        let mut remainder = vec![0u8; self.nsym];
        for &byte in data {
            let feedback = byte ^ remainder[0];
            for j in 0..self.nsym {
                let next = if j + 1 < self.nsym {
                    remainder[j + 1]
                } else {
                    0
                };
                remainder[j] = next ^ gf_mul(feedback, self.generator[j + 1]);
            }
        }
        let mut word = Vec::with_capacity(data.len() + self.nsym);
        word.extend_from_slice(data);
        word.extend_from_slice(&remainder);
        word
    }

    fn syndromes(&self, word: &[u8]) -> Vec<u8> {
        (0..self.nsym)
            .map(|j| {
                let root = gf_exp(j);
                word.iter().fold(0u8, |acc, &byte| gf_mul(acc, root) ^ byte)
            })
            .collect()
    }

    /// Corrects `word` in place, treating `erasures` as known-bad positions.
    ///
    /// Succeeds when `2 * errors + erasures <= parity_len()`. The corrected
    /// word is re-checked against zero syndromes before returning, so a
    /// success always yields a valid codeword. Returns the number of corrected
    /// positions.
    ///
    /// # Errors
    /// [`RsError::InvalidShape`] for malformed arguments and
    /// [`RsError::Uncorrectable`] when the word cannot be decoded.
    pub fn decode(&self, word: &mut [u8], erasures: &[usize]) -> Result<usize, RsError> {
        let n = word.len();
        if n <= self.nsym || n > 255 || erasures.len() > self.nsym {
            return Err(RsError::InvalidShape);
        }
        let mut seen = [false; 255];
        for &position in erasures {
            if position >= n || seen[position] {
                return Err(RsError::InvalidShape);
            }
            seen[position] = true;
        }
        let syndromes = self.syndromes(word);
        if syndromes.iter().all(|&s| s == 0) {
            return Ok(0);
        }
        let f = erasures.len();
        // Erasure locator Γ(x) = Π (1 + X_e x), lowest degree first.
        let mut gamma = vec![1u8];
        for &position in erasures {
            let x = gf_exp(n - 1 - position);
            gamma = poly_mul(&gamma, &[1, x]);
        }
        // Forney syndromes: the coefficients of S(x)Γ(x) from index f upward
        // are the syndromes of the error-only word.
        let mut forney = poly_mul(&syndromes, &gamma);
        forney.truncate(self.nsym);
        let errors_only = &forney[f..];
        let lambda = berlekamp_massey(errors_only);
        let error_count = lambda.len() - 1;
        if 2 * error_count + f > self.nsym {
            return Err(RsError::Uncorrectable);
        }
        let psi = poly_mul(&lambda, &gamma);
        let degree = psi.len() - 1;
        // Chien search over all positions.
        let mut positions = Vec::with_capacity(degree);
        for i in 0..n {
            let x_inv = gf_exp(255 - ((n - 1 - i) % 255));
            if poly_eval(&psi, x_inv) == 0 {
                positions.push(i);
            }
        }
        if positions.len() != degree {
            return Err(RsError::Uncorrectable);
        }
        // Ω(x) = S(x)Ψ(x) mod x^nsym.
        let mut omega = poly_mul(&syndromes, &psi);
        omega.truncate(self.nsym);
        // Formal derivative of Ψ in characteristic 2 keeps odd-degree terms.
        let derivative: Vec<u8> = psi
            .iter()
            .enumerate()
            .skip(1)
            .map(|(k, &c)| if k % 2 == 1 { c } else { 0 })
            .collect();
        let mut corrected = word.to_vec();
        for &i in &positions {
            let x = gf_exp(n - 1 - i);
            let x_inv = gf_inv(x);
            let numerator = poly_eval(&omega, x_inv);
            let denominator = poly_eval(&derivative, x_inv);
            if denominator == 0 {
                return Err(RsError::Uncorrectable);
            }
            corrected[i] ^= gf_mul(x, gf_div(numerator, denominator));
        }
        if self.syndromes(&corrected).iter().any(|&s| s != 0) {
            return Err(RsError::Uncorrectable);
        }
        word.copy_from_slice(&corrected);
        Ok(positions.len())
    }
}

/// Berlekamp–Massey over GF(256); returns the lowest-degree-first locator.
fn berlekamp_massey(syndromes: &[u8]) -> Vec<u8> {
    let n = syndromes.len();
    let mut c = vec![0u8; n + 1];
    let mut b = vec![0u8; n + 1];
    c[0] = 1;
    b[0] = 1;
    let mut l = 0usize;
    let mut m = 1usize;
    let mut previous_discrepancy = 1u8;
    for i in 0..n {
        let mut d = syndromes[i];
        for j in 1..=l {
            d ^= gf_mul(c[j], syndromes[i - j]);
        }
        if d == 0 {
            m += 1;
            continue;
        }
        let scale = gf_div(d, previous_discrepancy);
        if 2 * l <= i {
            let snapshot = c.clone();
            for j in 0..(n + 1).saturating_sub(m) {
                c[j + m] ^= gf_mul(scale, b[j]);
            }
            l = i + 1 - l;
            b = snapshot;
            previous_discrepancy = d;
            m = 1;
        } else {
            for j in 0..(n + 1).saturating_sub(m) {
                c[j + m] ^= gf_mul(scale, b[j]);
            }
            m += 1;
        }
    }
    c.truncate(l + 1);
    c
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tiny deterministic generator for test vectors.
    struct Lcg(u64);
    impl Lcg {
        fn next(&mut self) -> u32 {
            self.0 = self
                .0
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            (self.0 >> 33) as u32
        }
        fn byte(&mut self) -> u8 {
            self.next() as u8
        }
        fn below(&mut self, bound: usize) -> usize {
            (self.next() as usize) % bound
        }
    }

    #[test]
    fn matches_the_qr_hello_world_check_vector() {
        // QR Code version 1-M "HELLO WORLD": 16 data codewords, 10 EC codewords.
        let data = [
            32, 91, 11, 120, 209, 114, 220, 77, 67, 64, 236, 17, 236, 17, 236, 17,
        ];
        let expected = [196, 35, 39, 119, 235, 215, 231, 226, 93, 23];
        let word = ReedSolomon::new(10).encode(&data);
        assert_eq!(&word[..16], &data);
        assert_eq!(&word[16..], &expected);
    }

    #[test]
    fn corrects_random_errors_and_erasures_up_to_capacity() {
        let mut rng = Lcg(7);
        for &(k, nsym) in &[(16usize, 16usize), (60, 68), (12, 18), (13, 115)] {
            let rs = ReedSolomon::new(nsym);
            for _ in 0..60 {
                let data: Vec<u8> = (0..k).map(|_| rng.byte()).collect();
                let clean = rs.encode(&data);
                let n = clean.len();
                // pick f erasures and e errors with 2e + f <= nsym
                let f = rng.below(nsym.min(n - 1) + 1);
                let e = rng.below((nsym - f) / 2 + 1);
                let mut word = clean.clone();
                let mut positions: Vec<usize> = (0..n).collect();
                for i in 0..(f + e) {
                    let j = i + rng.below(n - i);
                    positions.swap(i, j);
                }
                let erased = &positions[..f];
                let errored = &positions[f..f + e];
                for &p in erased {
                    word[p] = rng.byte();
                }
                for &p in errored {
                    word[p] ^= rng.byte() | 1;
                }
                let corrected = rs.decode(&mut word, erased).expect("within capacity");
                assert!(corrected <= f + e);
                assert_eq!(word, clean, "k={k} nsym={nsym} f={f} e={e}");
            }
        }
    }

    #[test]
    fn rejects_words_beyond_capacity_without_returning_wrong_data() {
        let mut rng = Lcg(99);
        let rs = ReedSolomon::new(16);
        let mut wrong_accepts = 0;
        for _ in 0..200 {
            let data: Vec<u8> = (0..16).map(|_| rng.byte()).collect();
            let clean = rs.encode(&data);
            let mut word = clean.clone();
            // 20 random errors is far beyond t = 8
            let mut positions: Vec<usize> = (0..word.len()).collect();
            for i in 0..20 {
                let j = i + rng.below(word.len() - i);
                positions.swap(i, j);
            }
            for &p in &positions[..20] {
                word[p] ^= rng.byte() | 1;
            }
            if rs.decode(&mut word, &[]).is_ok() {
                // a miscorrection must at least be a valid codeword
                assert!(rs.syndromes(&word).iter().all(|&s| s == 0));
                if word != clean {
                    wrong_accepts += 1;
                }
            }
        }
        assert!(
            wrong_accepts <= 2,
            "miscorrection rate too high: {wrong_accepts}"
        );
    }

    #[test]
    fn rejects_malformed_arguments() {
        let rs = ReedSolomon::new(4);
        let mut short = [0u8; 4];
        assert_eq!(rs.decode(&mut short, &[]), Err(RsError::InvalidShape));
        let mut word = rs.encode(&[1, 2, 3]);
        assert_eq!(rs.decode(&mut word, &[9]), Err(RsError::InvalidShape));
        assert_eq!(rs.decode(&mut word, &[1, 1]), Err(RsError::InvalidShape));
    }
}
