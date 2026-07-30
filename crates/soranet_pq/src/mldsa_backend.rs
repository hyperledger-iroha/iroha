//! FIPS 204 key generation and signing over the crate's safe Rust primitives.

use sha3::{
    Shake256,
    digest::{ExtendableOutput, Update, XofReader},
};
use zeroize::Zeroizing;

use super::{
    ML_DSA_CONTEXT_MAX_LEN, MlDsaError, MlDsaKeyPair, MlDsaSuite,
    mldsa_primitives::{
        PolyVec, matrix_expand, matrix_pointwise, pack_pk, pack_sig, pack_sk, pack_w1,
        poly_challenge, poly_ntt, unpack_sk, vec_add, vec_caddq, vec_chknorm, vec_decompose,
        vec_invntt_tomont, vec_make_hint, vec_ntt, vec_pointwise, vec_power2round, vec_reduce,
        vec_sub, vec_uniform_eta, vec_uniform_gamma1, w1_packed_bytes,
    },
    validate_mldsa_secret_key_len,
};

const SEEDBYTES: usize = 32;
const CRHBYTES: usize = 64;
const TRBYTES: usize = 64;
const RNDBYTES: usize = 32;

pub(super) fn generate_keypair(
    suite: MlDsaSuite,
    coins: &[u8; SEEDBYTES],
) -> Result<MlDsaKeyPair, MlDsaError> {
    match suite {
        MlDsaSuite::MlDsa44 => mldsa44::generate_keypair(coins),
        MlDsaSuite::MlDsa65 => mldsa65::generate_keypair(coins),
        MlDsaSuite::MlDsa87 => mldsa87::generate_keypair(coins),
    }
}

pub(super) fn sign(
    suite: MlDsaSuite,
    secret_key: &[u8],
    context: &[u8],
    message: &[u8],
    coins: &[u8; RNDBYTES],
) -> Result<Vec<u8>, MlDsaError> {
    if context.len() > ML_DSA_CONTEXT_MAX_LEN {
        return Err(MlDsaError::ContextTooLong { len: context.len() });
    }

    match suite {
        MlDsaSuite::MlDsa44 => mldsa44::sign(secret_key, context, message, coins),
        MlDsaSuite::MlDsa65 => mldsa65::sign(secret_key, context, message, coins),
        MlDsaSuite::MlDsa87 => mldsa87::sign(secret_key, context, message, coins),
    }
}

pub(super) fn validate_secret_key(suite: MlDsaSuite, secret_key: &[u8]) -> Result<(), MlDsaError> {
    match suite {
        MlDsaSuite::MlDsa44 => mldsa44::validate_secret_key(secret_key).map(drop),
        MlDsaSuite::MlDsa65 => mldsa65::validate_secret_key(secret_key).map(drop),
        MlDsaSuite::MlDsa87 => mldsa87::validate_secret_key(secret_key).map(drop),
    }
}

pub(super) fn public_key_from_secret_key(
    suite: MlDsaSuite,
    secret_key: &[u8],
) -> Result<Vec<u8>, MlDsaError> {
    match suite {
        MlDsaSuite::MlDsa44 => mldsa44::validate_secret_key(secret_key),
        MlDsaSuite::MlDsa65 => mldsa65::validate_secret_key(secret_key),
        MlDsaSuite::MlDsa87 => mldsa87::validate_secret_key(secret_key),
    }
}

fn shake256_into(out: &mut [u8], inputs: &[&[u8]]) {
    let mut state = Shake256::default();
    for input in inputs {
        state.update(input);
    }
    state.finalize_xof().read(out);
}

macro_rules! mldsa_suite {
    (
        $module:ident,
        suite = $suite:expr,
        k = $k:expr,
        l = $l:expr,
        eta = $eta:expr,
        tau = $tau:expr,
        beta = $beta:expr,
        gamma1 = $gamma1:expr,
        gamma2 = $gamma2:expr,
        omega = $omega:expr,
        ctilde = $ctilde:expr
    ) => {
        mod $module {
            use super::*;

            const K: usize = $k;
            const L: usize = $l;
            const K_U8: u8 = $k;
            const L_U8: u8 = $l;
            const L_U16: u16 = $l;
            const ETA: i32 = $eta;
            const TAU: usize = $tau;
            const BETA: i32 = $beta;
            const GAMMA1: i32 = $gamma1;
            const GAMMA2: i32 = $gamma2;
            const OMEGA: usize = $omega;
            const CTILDEBYTES: usize = $ctilde;
            const T1_PACKEDBYTES: usize = 320;
            const T0_PACKEDBYTES: usize = 416;
            const POLYETA_PACKEDBYTES: usize = if ETA == 2 { 96 } else { 128 };
            const POLYZ_PACKEDBYTES: usize = if GAMMA1 == 1 << 17 { 576 } else { 640 };
            const PUBLIC_KEY_BYTES: usize = SEEDBYTES + K * T1_PACKEDBYTES;
            const SECRET_KEY_BYTES: usize =
                2 * SEEDBYTES + TRBYTES + (L + K) * POLYETA_PACKEDBYTES + K * T0_PACKEDBYTES;
            const SIGNATURE_BYTES: usize = CTILDEBYTES + L * POLYZ_PACKEDBYTES + OMEGA + K;

            type Polyvecl = PolyVec<L>;
            type Polyveck = PolyVec<K>;

            pub(super) fn generate_keypair(
                coins: &[u8; SEEDBYTES],
            ) -> Result<MlDsaKeyPair, MlDsaError> {
                let mut seed_input = Zeroizing::new([0_u8; SEEDBYTES + 2]);
                seed_input[..SEEDBYTES].copy_from_slice(coins);
                seed_input[SEEDBYTES] = K_U8;
                seed_input[SEEDBYTES + 1] = L_U8;

                let mut expanded = Zeroizing::new([0_u8; 2 * SEEDBYTES + CRHBYTES]);
                shake256_into(expanded.as_mut(), &[seed_input.as_ref()]);
                let mut rho = Zeroizing::new([0_u8; SEEDBYTES]);
                rho.copy_from_slice(&expanded[..SEEDBYTES]);
                let mut rhoprime = Zeroizing::new([0_u8; CRHBYTES]);
                rhoprime.copy_from_slice(&expanded[SEEDBYTES..SEEDBYTES + CRHBYTES]);
                let mut key = Zeroizing::new([0_u8; SEEDBYTES]);
                key.copy_from_slice(&expanded[SEEDBYTES + CRHBYTES..]);

                let matrix = matrix_expand::<K, L>(&rho);
                let s1 = vec_uniform_eta::<L>(ETA, &rhoprime, 0);
                let s2 = vec_uniform_eta::<K>(ETA, &rhoprime, L_U16);

                let mut s1hat = s1.clone();
                vec_ntt(&mut s1hat);
                let mut t = matrix_pointwise(&matrix, &s1hat);
                vec_reduce(&mut t);
                vec_invntt_tomont(&mut t);
                t = vec_add(&t, &s2);
                vec_caddq(&mut t);
                let (t1, t0) = vec_power2round(&t);

                let mut public_key = vec![0_u8; PUBLIC_KEY_BYTES];
                pack_pk(&mut public_key, &rho, &t1);

                let mut tr = Zeroizing::new([0_u8; TRBYTES]);
                shake256_into(tr.as_mut(), &[&public_key]);

                let mut secret_key = Zeroizing::new(vec![0_u8; SECRET_KEY_BYTES]);
                pack_sk(secret_key.as_mut(), ETA, &rho, &*tr, &key, &t0, &s1, &s2);

                Ok(MlDsaKeyPair {
                    public_key,
                    secret_key,
                })
            }

            pub(super) fn validate_secret_key(secret_key: &[u8]) -> Result<Vec<u8>, MlDsaError> {
                validate_mldsa_secret_key_len($suite, secret_key)?;

                let mut rho = Zeroizing::new([0_u8; SEEDBYTES]);
                let mut tr = Zeroizing::new([0_u8; TRBYTES]);
                let mut key = Zeroizing::new([0_u8; SEEDBYTES]);
                let mut t0 = Polyveck::default();
                let mut s1 = Polyvecl::default();
                let mut s2 = Polyveck::default();
                if !unpack_sk(
                    secret_key, ETA, &mut *rho, &mut *tr, &mut *key, &mut t0, &mut s1, &mut s2,
                ) {
                    return Err(MlDsaError::SecretKeyMismatch {
                        suite: $suite,
                        kind: "s1 or s2 contains a coefficient outside the FIPS 204 range",
                    });
                }

                let (public_key, recomputed_t0) = public_and_t0_from_secret_parts(&*rho, &s1, &s2);
                if recomputed_t0 != t0 {
                    return Err(MlDsaError::SecretKeyMismatch {
                        suite: $suite,
                        kind: "t0 does not match rho, s1, and s2",
                    });
                }

                let mut expected_tr = Zeroizing::new([0_u8; TRBYTES]);
                shake256_into(expected_tr.as_mut(), &[&public_key]);
                if expected_tr.as_ref() != tr.as_ref() {
                    return Err(MlDsaError::SecretKeyMismatch {
                        suite: $suite,
                        kind: "tr does not match reconstructed public key",
                    });
                }

                let mut canonical = Zeroizing::new(vec![0_u8; SECRET_KEY_BYTES]);
                pack_sk(canonical.as_mut(), ETA, &*rho, &*tr, &*key, &t0, &s1, &s2);
                if canonical.as_slice() != secret_key {
                    return Err(MlDsaError::SecretKeyMismatch {
                        suite: $suite,
                        kind: "secret key is not canonically encoded",
                    });
                }

                Ok(public_key)
            }

            fn public_and_t0_from_secret_parts(
                rho: &[u8; SEEDBYTES],
                s1: &Polyvecl,
                s2: &Polyveck,
            ) -> (Vec<u8>, Polyveck) {
                let matrix = matrix_expand::<K, L>(rho);
                let mut s1hat = s1.clone();
                vec_ntt(&mut s1hat);
                let mut t = matrix_pointwise(&matrix, &s1hat);
                vec_reduce(&mut t);
                vec_invntt_tomont(&mut t);
                t = vec_add(&t, s2);
                vec_caddq(&mut t);
                let (t1, t0) = vec_power2round(&t);

                let mut public_key = vec![0_u8; PUBLIC_KEY_BYTES];
                pack_pk(&mut public_key, rho, &t1);
                (public_key, t0)
            }

            #[allow(clippy::too_many_lines)]
            pub(super) fn sign(
                secret_key: &[u8],
                context: &[u8],
                message: &[u8],
                coins: &[u8; RNDBYTES],
            ) -> Result<Vec<u8>, MlDsaError> {
                validate_mldsa_secret_key_len($suite, secret_key)?;

                let mut rho = Zeroizing::new([0_u8; SEEDBYTES]);
                let mut tr = Zeroizing::new([0_u8; TRBYTES]);
                let mut key = Zeroizing::new([0_u8; SEEDBYTES]);
                let mut t0 = Polyveck::default();
                let mut s1 = Polyvecl::default();
                let mut s2 = Polyveck::default();
                if !unpack_sk(
                    secret_key, ETA, &mut *rho, &mut *tr, &mut *key, &mut t0, &mut s1, &mut s2,
                ) {
                    return Err(MlDsaError::SecretKeyMismatch {
                        suite: $suite,
                        kind: "s1 or s2 contains a coefficient outside the FIPS 204 range",
                    });
                }

                let context_len = u8::try_from(context.len())
                    .map_err(|_| MlDsaError::ContextTooLong { len: context.len() })?;
                let context_header = [0_u8, context_len];
                let mut mu = Zeroizing::new([0_u8; CRHBYTES]);
                shake256_into(
                    mu.as_mut(),
                    &[tr.as_ref(), &context_header, context, message],
                );
                let mut rhoprime = Zeroizing::new([0_u8; CRHBYTES]);
                shake256_into(rhoprime.as_mut(), &[key.as_ref(), coins, mu.as_ref()]);

                let matrix = matrix_expand::<K, L>(&*rho);
                vec_ntt(&mut s1);
                vec_ntt(&mut s2);
                vec_ntt(&mut t0);

                let mut nonce = 0_u16;
                loop {
                    let y = vec_uniform_gamma1::<L>(GAMMA1, &*rhoprime, nonce);
                    nonce = nonce.wrapping_add(1);

                    let mut z = y.clone();
                    vec_ntt(&mut z);
                    let mut w = matrix_pointwise(&matrix, &z);
                    vec_reduce(&mut w);
                    vec_invntt_tomont(&mut w);
                    vec_caddq(&mut w);
                    let (w1, mut w0) = vec_decompose(GAMMA2, &w);

                    let polyw1_bytes = w1_packed_bytes(GAMMA2);
                    let mut w1_packed = Zeroizing::new(vec![0_u8; K * polyw1_bytes]);
                    pack_w1(w1_packed.as_mut(), GAMMA2, &w1);

                    let mut c_tilde = Zeroizing::new([0_u8; CTILDEBYTES]);
                    shake256_into(c_tilde.as_mut(), &[mu.as_ref(), w1_packed.as_ref()]);

                    let mut challenge = poly_challenge(TAU, c_tilde.as_ref());
                    poly_ntt(&mut challenge);

                    z = vec_pointwise(&challenge, &s1);
                    vec_invntt_tomont(&mut z);
                    z = vec_add(&z, &y);
                    vec_reduce(&mut z);
                    if vec_chknorm(&z, GAMMA1 - BETA) {
                        continue;
                    }

                    let mut h = vec_pointwise(&challenge, &s2);
                    vec_invntt_tomont(&mut h);
                    w0 = vec_sub(&w0, &h);
                    vec_reduce(&mut w0);
                    if vec_chknorm(&w0, GAMMA2 - BETA) {
                        continue;
                    }

                    h = vec_pointwise(&challenge, &t0);
                    vec_invntt_tomont(&mut h);
                    vec_reduce(&mut h);
                    if vec_chknorm(&h, GAMMA2) {
                        continue;
                    }

                    w0 = vec_add(&w0, &h);
                    let (hint, hint_count) = vec_make_hint(GAMMA2, &w0, &w1);
                    if hint_count > OMEGA {
                        continue;
                    }

                    let mut signature = vec![0_u8; SIGNATURE_BYTES];
                    pack_sig(&mut signature, c_tilde.as_ref(), GAMMA1, OMEGA, &z, &hint);
                    return Ok(signature);
                }
            }
        }
    };
}

mldsa_suite!(
    mldsa44,
    suite = MlDsaSuite::MlDsa44,
    k = 4,
    l = 4,
    eta = 2,
    tau = 39,
    beta = 78,
    gamma1 = 1 << 17,
    gamma2 = (8_380_417 - 1) / 88,
    omega = 80,
    ctilde = 32
);

mldsa_suite!(
    mldsa65,
    suite = MlDsaSuite::MlDsa65,
    k = 6,
    l = 5,
    eta = 4,
    tau = 49,
    beta = 196,
    gamma1 = 1 << 19,
    gamma2 = (8_380_417 - 1) / 32,
    omega = 55,
    ctilde = 48
);

mldsa_suite!(
    mldsa87,
    suite = MlDsaSuite::MlDsa87,
    k = 8,
    l = 7,
    eta = 2,
    tau = 60,
    beta = 120,
    gamma1 = 1 << 19,
    gamma2 = (8_380_417 - 1) / 32,
    omega = 75,
    ctilde = 64
);
