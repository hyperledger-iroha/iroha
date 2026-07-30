use sha3::{
    Shake256,
    digest::{ExtendableOutput, Update, XofReader},
};
use zeroize::Zeroizing;

use super::{
    ML_DSA_CONTEXT_MAX_LEN, MlDsaError, MlDsaKeyPair, MlDsaSuite, validate_mldsa_secret_key_len,
};

// SAFETY INVARIANT:
// `pqclean_bindings` is generated from the exact pinned PQClean headers and is
// the only module that declares their private C ABI. Every foreign call below
// receives one of those generated `repr(C)` types, an array whose length is a
// FIPS 204 constant, or a byte buffer sized by a generated header constant.
// The safe public API validates encoded lengths before this module unpacks
// caller-controlled key material.

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
    let mut h = Shake256::default();
    for input in inputs {
        h.update(input);
    }
    h.finalize_xof().read(out);
}

macro_rules! mldsa_suite {
    (
        $module:ident,
        $suite:expr,
        bindings = $bindings:ident,
        matrix_expand = $matrix_expand:ident,
        vecl_uniform_eta = $vecl_uniform_eta:ident,
        veck_uniform_eta = $veck_uniform_eta:ident,
        vecl_uniform_gamma1 = $vecl_uniform_gamma1:ident,
        vecl_ntt = $vecl_ntt:ident,
        vecl_invntt = $vecl_invntt:ident,
        vecl_pointwise = $vecl_pointwise:ident,
        vecl_add = $vecl_add:ident,
        vecl_reduce = $vecl_reduce:ident,
        vecl_chknorm = $vecl_chknorm:ident,
        veck_ntt = $veck_ntt:ident,
        veck_invntt = $veck_invntt:ident,
        veck_pointwise = $veck_pointwise:ident,
        veck_reduce = $veck_reduce:ident,
        veck_chknorm = $veck_chknorm:ident,
        veck_caddq = $veck_caddq:ident,
        veck_add = $veck_add:ident,
        veck_sub = $veck_sub:ident,
        veck_power2round = $veck_power2round:ident,
        veck_decompose = $veck_decompose:ident,
        veck_make_hint = $veck_make_hint:ident,
        veck_pack_w1 = $veck_pack_w1:ident,
        matrix_pointwise = $matrix_pointwise:ident,
        poly_challenge = $poly_challenge:ident,
        poly_ntt = $poly_ntt:ident,
        pack_pk = $pack_pk:ident,
        pack_sk = $pack_sk:ident,
        pack_sig = $pack_sig:ident,
        unpack_sk = $unpack_sk:ident
    ) => {
        mod $module {
            use super::super::pqclean_bindings::$bindings::*;
            use super::*;

            pub(super) fn generate_keypair(
                coins: &[u8; SEEDBYTES],
            ) -> Result<MlDsaKeyPair, MlDsaError> {
                let mut seed_input = Zeroizing::new([0u8; SEEDBYTES + 2]);
                seed_input[..SEEDBYTES].copy_from_slice(coins);
                seed_input[SEEDBYTES] = K_DOMAIN_BYTE;
                seed_input[SEEDBYTES + 1] = L_DOMAIN_BYTE;

                let mut expanded = Zeroizing::new([0u8; (2 * SEEDBYTES) + CRHBYTES]);
                shake256_into(expanded.as_mut(), &[seed_input.as_ref()]);
                let rho = &expanded[..SEEDBYTES];
                let rhoprime = &expanded[SEEDBYTES..SEEDBYTES + CRHBYTES];
                let key = &expanded[SEEDBYTES + CRHBYTES..];

                let mut mat = Zeroizing::new(vec![Polyvecl::default(); K]);
                let mut s1 = Polyvecl::default();
                let mut s2 = Polyveck::default();
                let mut s1hat;
                let mut t1 = Polyveck::default();
                let mut t0 = Polyveck::default();

                unsafe {
                    $matrix_expand(mat.as_mut_ptr(), rho.as_ptr());
                    $vecl_uniform_eta(&mut s1, rhoprime.as_ptr(), 0);
                    $veck_uniform_eta(&mut s2, rhoprime.as_ptr(), L_NONCE);
                }

                s1hat = s1.clone();
                unsafe {
                    $vecl_ntt(&mut s1hat);
                    $matrix_pointwise(&mut t1, mat.as_ptr(), &s1hat);
                    $veck_reduce(&mut t1);
                    $veck_invntt(&mut t1);
                }

                let t1_before_add = t1.clone();
                unsafe {
                    $veck_add(&mut t1, &t1_before_add, &s2);
                    $veck_caddq(&mut t1);
                }

                let t1_unrounded = t1.clone();
                unsafe {
                    $veck_power2round(&mut t1, &mut t0, &t1_unrounded);
                }

                let mut public_key = vec![0u8; PUBLIC_KEY_BYTES];
                unsafe {
                    $pack_pk(public_key.as_mut_ptr(), rho.as_ptr(), &t1);
                }

                let mut tr = Zeroizing::new([0u8; TRBYTES]);
                shake256_into(tr.as_mut(), &[&public_key]);

                let mut secret_key = Zeroizing::new(vec![0u8; SECRET_KEY_BYTES]);
                unsafe {
                    $pack_sk(
                        secret_key.as_mut_ptr(),
                        rho.as_ptr(),
                        tr.as_ptr(),
                        key.as_ptr(),
                        &t0,
                        &s1,
                        &s2,
                    );
                }

                Ok(MlDsaKeyPair {
                    public_key,
                    secret_key,
                })
            }

            pub(super) fn validate_secret_key(secret_key: &[u8]) -> Result<Vec<u8>, MlDsaError> {
                validate_mldsa_secret_key_len($suite, secret_key)?;

                let mut rho = Zeroizing::new([0u8; SEEDBYTES]);
                let mut tr = Zeroizing::new([0u8; TRBYTES]);
                let mut key = Zeroizing::new([0u8; SEEDBYTES]);
                let mut t0 = Polyveck::default();
                let mut s1 = Polyvecl::default();
                let mut s2 = Polyveck::default();

                unsafe {
                    $unpack_sk(
                        rho.as_mut_ptr(),
                        tr.as_mut_ptr(),
                        key.as_mut_ptr(),
                        &mut t0,
                        &mut s1,
                        &mut s2,
                        secret_key.as_ptr(),
                    );
                }

                let (public_key, recomputed_t0) =
                    public_and_t0_from_secret_parts(rho.as_ref(), &s1, &s2);
                if recomputed_t0 != t0 {
                    return Err(MlDsaError::SecretKeyMismatch {
                        suite: $suite,
                        kind: "t0 does not match rho, s1, and s2",
                    });
                }

                let mut expected_tr = Zeroizing::new([0u8; TRBYTES]);
                shake256_into(expected_tr.as_mut(), &[&public_key]);
                if expected_tr.as_ref() != tr.as_ref() {
                    return Err(MlDsaError::SecretKeyMismatch {
                        suite: $suite,
                        kind: "tr does not match reconstructed public key",
                    });
                }

                let mut canonical = Zeroizing::new(vec![0u8; SECRET_KEY_BYTES]);
                unsafe {
                    $pack_sk(
                        canonical.as_mut_ptr(),
                        rho.as_ptr(),
                        tr.as_ptr(),
                        key.as_ptr(),
                        &t0,
                        &s1,
                        &s2,
                    );
                }
                if canonical.as_slice() != secret_key {
                    return Err(MlDsaError::SecretKeyMismatch {
                        suite: $suite,
                        kind: "secret key is not canonically encoded",
                    });
                }

                Ok(public_key)
            }

            fn public_and_t0_from_secret_parts(
                rho: &[u8],
                s1: &Polyvecl,
                s2: &Polyveck,
            ) -> (Vec<u8>, Polyveck) {
                let mut mat = Zeroizing::new(vec![Polyvecl::default(); K]);
                let mut s1hat = s1.clone();
                let mut t1 = Polyveck::default();
                let mut t0 = Polyveck::default();

                unsafe {
                    $matrix_expand(mat.as_mut_ptr(), rho.as_ptr());
                    $vecl_ntt(&mut s1hat);
                    $matrix_pointwise(&mut t1, mat.as_ptr(), &s1hat);
                    $veck_reduce(&mut t1);
                    $veck_invntt(&mut t1);
                }

                let t1_before_add = t1.clone();
                unsafe {
                    $veck_add(&mut t1, &t1_before_add, s2);
                    $veck_caddq(&mut t1);
                }

                let t1_unrounded = t1.clone();
                unsafe {
                    $veck_power2round(&mut t1, &mut t0, &t1_unrounded);
                }

                let mut public_key = vec![0u8; PUBLIC_KEY_BYTES];
                unsafe {
                    $pack_pk(public_key.as_mut_ptr(), rho.as_ptr(), &t1);
                }

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

                let mut rho = Zeroizing::new([0u8; SEEDBYTES]);
                let mut tr = Zeroizing::new([0u8; TRBYTES]);
                let mut key = Zeroizing::new([0u8; SEEDBYTES]);
                let mut mu = Zeroizing::new([0u8; CRHBYTES]);
                let mut rhoprime = Zeroizing::new([0u8; CRHBYTES]);
                let mut t0 = Polyveck::default();
                let mut s1 = Polyvecl::default();
                let mut s2 = Polyveck::default();

                unsafe {
                    $unpack_sk(
                        rho.as_mut_ptr(),
                        tr.as_mut_ptr(),
                        key.as_mut_ptr(),
                        &mut t0,
                        &mut s1,
                        &mut s2,
                        secret_key.as_ptr(),
                    );
                }

                let context_len = u8::try_from(context.len())
                    .map_err(|_| MlDsaError::ContextTooLong { len: context.len() })?;
                let context_header = [0u8, context_len];
                shake256_into(
                    mu.as_mut(),
                    &[tr.as_ref(), &context_header, context, message],
                );
                shake256_into(rhoprime.as_mut(), &[key.as_ref(), coins, mu.as_ref()]);

                let mut mat = Zeroizing::new(vec![Polyvecl::default(); K]);
                unsafe {
                    $matrix_expand(mat.as_mut_ptr(), rho.as_ptr());
                    $vecl_ntt(&mut s1);
                    $veck_ntt(&mut s2);
                    $veck_ntt(&mut t0);
                }

                let mut sig = vec![0u8; SIGNATURE_BYTES];
                let mut nonce = 0u16;

                loop {
                    let mut y = Polyvecl::default();
                    let mut z;
                    let mut w1 = Polyveck::default();
                    let mut w0 = Polyveck::default();
                    let mut h = Polyveck::default();
                    let mut cp = Poly::default();

                    unsafe {
                        $vecl_uniform_gamma1(&mut y, rhoprime.as_ptr(), nonce);
                    }
                    nonce = nonce.wrapping_add(1);

                    z = y.clone();
                    unsafe {
                        $vecl_ntt(&mut z);
                        $matrix_pointwise(&mut w1, mat.as_ptr(), &z);
                        $veck_reduce(&mut w1);
                        $veck_invntt(&mut w1);
                        $veck_caddq(&mut w1);
                    }

                    let w1_before_decompose = w1.clone();
                    unsafe {
                        $veck_decompose(&mut w1, &mut w0, &w1_before_decompose);
                    }

                    let mut w1_packed = Zeroizing::new(vec![0u8; K * POLYW1_PACKEDBYTES]);
                    unsafe {
                        $veck_pack_w1(w1_packed.as_mut_ptr(), &w1);
                    }

                    let mut c_tilde = Zeroizing::new([0u8; CTILDEBYTES]);
                    shake256_into(c_tilde.as_mut(), &[mu.as_ref(), w1_packed.as_ref()]);
                    sig[..CTILDEBYTES].copy_from_slice(c_tilde.as_ref());

                    unsafe {
                        $poly_challenge(&mut cp, sig.as_ptr());
                        $poly_ntt(&mut cp);
                        $vecl_pointwise(&mut z, &cp, &s1);
                        $vecl_invntt(&mut z);
                    }

                    let z_before_add = z.clone();
                    unsafe {
                        $vecl_add(&mut z, &z_before_add, &y);
                        $vecl_reduce(&mut z);
                    }
                    if (unsafe { $vecl_chknorm(&z, GAMMA1 - BETA) }) != 0 {
                        continue;
                    }

                    unsafe {
                        $veck_pointwise(&mut h, &cp, &s2);
                        $veck_invntt(&mut h);
                    }
                    let w0_before_sub = w0.clone();
                    unsafe {
                        $veck_sub(&mut w0, &w0_before_sub, &h);
                        $veck_reduce(&mut w0);
                    }
                    if (unsafe { $veck_chknorm(&w0, GAMMA2 - BETA) }) != 0 {
                        continue;
                    }

                    unsafe {
                        $veck_pointwise(&mut h, &cp, &t0);
                        $veck_invntt(&mut h);
                        $veck_reduce(&mut h);
                    }
                    if (unsafe { $veck_chknorm(&h, GAMMA2) }) != 0 {
                        continue;
                    }

                    let w0_before_add = w0.clone();
                    unsafe {
                        $veck_add(&mut w0, &w0_before_add, &h);
                    }
                    let hints = unsafe { $veck_make_hint(&mut h, &w0, &w1) };
                    if hints > OMEGA {
                        continue;
                    }

                    unsafe {
                        $pack_sig(sig.as_mut_ptr(), sig.as_ptr(), &z, &h);
                    }
                    return Ok(sig);
                }
            }
        }
    };
}

mldsa_suite!(
    mldsa44,
    MlDsaSuite::MlDsa44,
    bindings = mldsa44,
    matrix_expand = PQCLEAN_MLDSA44_CLEAN_polyvec_matrix_expand,
    vecl_uniform_eta = PQCLEAN_MLDSA44_CLEAN_polyvecl_uniform_eta,
    veck_uniform_eta = PQCLEAN_MLDSA44_CLEAN_polyveck_uniform_eta,
    vecl_uniform_gamma1 = PQCLEAN_MLDSA44_CLEAN_polyvecl_uniform_gamma1,
    vecl_ntt = PQCLEAN_MLDSA44_CLEAN_polyvecl_ntt,
    vecl_invntt = PQCLEAN_MLDSA44_CLEAN_polyvecl_invntt_tomont,
    vecl_pointwise = PQCLEAN_MLDSA44_CLEAN_polyvecl_pointwise_poly_montgomery,
    vecl_add = PQCLEAN_MLDSA44_CLEAN_polyvecl_add,
    vecl_reduce = PQCLEAN_MLDSA44_CLEAN_polyvecl_reduce,
    vecl_chknorm = PQCLEAN_MLDSA44_CLEAN_polyvecl_chknorm,
    veck_ntt = PQCLEAN_MLDSA44_CLEAN_polyveck_ntt,
    veck_invntt = PQCLEAN_MLDSA44_CLEAN_polyveck_invntt_tomont,
    veck_pointwise = PQCLEAN_MLDSA44_CLEAN_polyveck_pointwise_poly_montgomery,
    veck_reduce = PQCLEAN_MLDSA44_CLEAN_polyveck_reduce,
    veck_chknorm = PQCLEAN_MLDSA44_CLEAN_polyveck_chknorm,
    veck_caddq = PQCLEAN_MLDSA44_CLEAN_polyveck_caddq,
    veck_add = PQCLEAN_MLDSA44_CLEAN_polyveck_add,
    veck_sub = PQCLEAN_MLDSA44_CLEAN_polyveck_sub,
    veck_power2round = PQCLEAN_MLDSA44_CLEAN_polyveck_power2round,
    veck_decompose = PQCLEAN_MLDSA44_CLEAN_polyveck_decompose,
    veck_make_hint = PQCLEAN_MLDSA44_CLEAN_polyveck_make_hint,
    veck_pack_w1 = PQCLEAN_MLDSA44_CLEAN_polyveck_pack_w1,
    matrix_pointwise = PQCLEAN_MLDSA44_CLEAN_polyvec_matrix_pointwise_montgomery,
    poly_challenge = PQCLEAN_MLDSA44_CLEAN_poly_challenge,
    poly_ntt = PQCLEAN_MLDSA44_CLEAN_poly_ntt,
    pack_pk = PQCLEAN_MLDSA44_CLEAN_pack_pk,
    pack_sk = PQCLEAN_MLDSA44_CLEAN_pack_sk,
    pack_sig = PQCLEAN_MLDSA44_CLEAN_pack_sig,
    unpack_sk = PQCLEAN_MLDSA44_CLEAN_unpack_sk
);

mldsa_suite!(
    mldsa65,
    MlDsaSuite::MlDsa65,
    bindings = mldsa65,
    matrix_expand = PQCLEAN_MLDSA65_CLEAN_polyvec_matrix_expand,
    vecl_uniform_eta = PQCLEAN_MLDSA65_CLEAN_polyvecl_uniform_eta,
    veck_uniform_eta = PQCLEAN_MLDSA65_CLEAN_polyveck_uniform_eta,
    vecl_uniform_gamma1 = PQCLEAN_MLDSA65_CLEAN_polyvecl_uniform_gamma1,
    vecl_ntt = PQCLEAN_MLDSA65_CLEAN_polyvecl_ntt,
    vecl_invntt = PQCLEAN_MLDSA65_CLEAN_polyvecl_invntt_tomont,
    vecl_pointwise = PQCLEAN_MLDSA65_CLEAN_polyvecl_pointwise_poly_montgomery,
    vecl_add = PQCLEAN_MLDSA65_CLEAN_polyvecl_add,
    vecl_reduce = PQCLEAN_MLDSA65_CLEAN_polyvecl_reduce,
    vecl_chknorm = PQCLEAN_MLDSA65_CLEAN_polyvecl_chknorm,
    veck_ntt = PQCLEAN_MLDSA65_CLEAN_polyveck_ntt,
    veck_invntt = PQCLEAN_MLDSA65_CLEAN_polyveck_invntt_tomont,
    veck_pointwise = PQCLEAN_MLDSA65_CLEAN_polyveck_pointwise_poly_montgomery,
    veck_reduce = PQCLEAN_MLDSA65_CLEAN_polyveck_reduce,
    veck_chknorm = PQCLEAN_MLDSA65_CLEAN_polyveck_chknorm,
    veck_caddq = PQCLEAN_MLDSA65_CLEAN_polyveck_caddq,
    veck_add = PQCLEAN_MLDSA65_CLEAN_polyveck_add,
    veck_sub = PQCLEAN_MLDSA65_CLEAN_polyveck_sub,
    veck_power2round = PQCLEAN_MLDSA65_CLEAN_polyveck_power2round,
    veck_decompose = PQCLEAN_MLDSA65_CLEAN_polyveck_decompose,
    veck_make_hint = PQCLEAN_MLDSA65_CLEAN_polyveck_make_hint,
    veck_pack_w1 = PQCLEAN_MLDSA65_CLEAN_polyveck_pack_w1,
    matrix_pointwise = PQCLEAN_MLDSA65_CLEAN_polyvec_matrix_pointwise_montgomery,
    poly_challenge = PQCLEAN_MLDSA65_CLEAN_poly_challenge,
    poly_ntt = PQCLEAN_MLDSA65_CLEAN_poly_ntt,
    pack_pk = PQCLEAN_MLDSA65_CLEAN_pack_pk,
    pack_sk = PQCLEAN_MLDSA65_CLEAN_pack_sk,
    pack_sig = PQCLEAN_MLDSA65_CLEAN_pack_sig,
    unpack_sk = PQCLEAN_MLDSA65_CLEAN_unpack_sk
);

mldsa_suite!(
    mldsa87,
    MlDsaSuite::MlDsa87,
    bindings = mldsa87,
    matrix_expand = PQCLEAN_MLDSA87_CLEAN_polyvec_matrix_expand,
    vecl_uniform_eta = PQCLEAN_MLDSA87_CLEAN_polyvecl_uniform_eta,
    veck_uniform_eta = PQCLEAN_MLDSA87_CLEAN_polyveck_uniform_eta,
    vecl_uniform_gamma1 = PQCLEAN_MLDSA87_CLEAN_polyvecl_uniform_gamma1,
    vecl_ntt = PQCLEAN_MLDSA87_CLEAN_polyvecl_ntt,
    vecl_invntt = PQCLEAN_MLDSA87_CLEAN_polyvecl_invntt_tomont,
    vecl_pointwise = PQCLEAN_MLDSA87_CLEAN_polyvecl_pointwise_poly_montgomery,
    vecl_add = PQCLEAN_MLDSA87_CLEAN_polyvecl_add,
    vecl_reduce = PQCLEAN_MLDSA87_CLEAN_polyvecl_reduce,
    vecl_chknorm = PQCLEAN_MLDSA87_CLEAN_polyvecl_chknorm,
    veck_ntt = PQCLEAN_MLDSA87_CLEAN_polyveck_ntt,
    veck_invntt = PQCLEAN_MLDSA87_CLEAN_polyveck_invntt_tomont,
    veck_pointwise = PQCLEAN_MLDSA87_CLEAN_polyveck_pointwise_poly_montgomery,
    veck_reduce = PQCLEAN_MLDSA87_CLEAN_polyveck_reduce,
    veck_chknorm = PQCLEAN_MLDSA87_CLEAN_polyveck_chknorm,
    veck_caddq = PQCLEAN_MLDSA87_CLEAN_polyveck_caddq,
    veck_add = PQCLEAN_MLDSA87_CLEAN_polyveck_add,
    veck_sub = PQCLEAN_MLDSA87_CLEAN_polyveck_sub,
    veck_power2round = PQCLEAN_MLDSA87_CLEAN_polyveck_power2round,
    veck_decompose = PQCLEAN_MLDSA87_CLEAN_polyveck_decompose,
    veck_make_hint = PQCLEAN_MLDSA87_CLEAN_polyveck_make_hint,
    veck_pack_w1 = PQCLEAN_MLDSA87_CLEAN_polyveck_pack_w1,
    matrix_pointwise = PQCLEAN_MLDSA87_CLEAN_polyvec_matrix_pointwise_montgomery,
    poly_challenge = PQCLEAN_MLDSA87_CLEAN_poly_challenge,
    poly_ntt = PQCLEAN_MLDSA87_CLEAN_poly_ntt,
    pack_pk = PQCLEAN_MLDSA87_CLEAN_pack_pk,
    pack_sk = PQCLEAN_MLDSA87_CLEAN_pack_sk,
    pack_sig = PQCLEAN_MLDSA87_CLEAN_pack_sig,
    unpack_sk = PQCLEAN_MLDSA87_CLEAN_unpack_sk
);
