use core::{array, ffi::c_int};

use sha3::{
    Shake256,
    digest::{ExtendableOutput, Update, XofReader},
};
use zeroize::{Zeroize, ZeroizeOnDrop, Zeroizing};

use super::{
    ML_DSA_CONTEXT_MAX_LEN, MlDsaError, MlDsaKeyPair, MlDsaSuite, validate_mldsa_secret_key_len,
};

const SEEDBYTES: usize = 32;
const CRHBYTES: usize = 64;
const TRBYTES: usize = 64;
const RNDBYTES: usize = 32;
const N: usize = 256;

#[repr(C)]
#[derive(Clone, PartialEq, Eq, Zeroize, ZeroizeOnDrop)]
struct Poly {
    coeffs: [i32; N],
}

impl Default for Poly {
    fn default() -> Self {
        Self { coeffs: [0; N] }
    }
}

#[repr(C)]
#[derive(Clone, Zeroize, ZeroizeOnDrop)]
struct Polyvecl<const L: usize> {
    vec: [Poly; L],
}

impl<const L: usize> Default for Polyvecl<L> {
    fn default() -> Self {
        Self {
            vec: array::from_fn(|_| Poly::default()),
        }
    }
}

#[repr(C)]
#[derive(Clone, PartialEq, Eq, Zeroize, ZeroizeOnDrop)]
struct Polyveck<const K: usize> {
    vec: [Poly; K],
}

impl<const K: usize> Default for Polyveck<K> {
    fn default() -> Self {
        Self {
            vec: array::from_fn(|_| Poly::default()),
        }
    }
}

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
        k = $k:expr,
        l = $l:expr,
        beta = $beta:expr,
        gamma1 = $gamma1:expr,
        gamma2 = $gamma2:expr,
        omega = $omega:expr,
        ctilde = $ctilde:expr,
        polyw1 = $polyw1:expr,
        pk_len = $pk_len:expr,
        sk_len = $sk_len:expr,
        sig_len = $sig_len:expr,
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
            use super::*;

            pub(super) fn generate_keypair(
                coins: &[u8; SEEDBYTES],
            ) -> Result<MlDsaKeyPair, MlDsaError> {
                let mut seed_input = Zeroizing::new([0u8; SEEDBYTES + 2]);
                seed_input[..SEEDBYTES].copy_from_slice(coins);
                seed_input[SEEDBYTES] = $k as u8;
                seed_input[SEEDBYTES + 1] = $l as u8;

                let mut expanded = Zeroizing::new([0u8; (2 * SEEDBYTES) + CRHBYTES]);
                shake256_into(expanded.as_mut(), &[seed_input.as_ref()]);
                let rho = &expanded[..SEEDBYTES];
                let rhoprime = &expanded[SEEDBYTES..SEEDBYTES + CRHBYTES];
                let key = &expanded[SEEDBYTES + CRHBYTES..];

                let mut mat = Zeroizing::new(vec![Polyvecl::<$l>::default(); $k]);
                let mut s1 = Polyvecl::<$l>::default();
                let mut s2 = Polyveck::<$k>::default();
                let mut s1hat;
                let mut t1 = Polyveck::<$k>::default();
                let mut t0 = Polyveck::<$k>::default();

                unsafe {
                    $matrix_expand(mat.as_mut_ptr(), rho.as_ptr());
                    $vecl_uniform_eta(&mut s1, rhoprime.as_ptr(), 0);
                    $veck_uniform_eta(&mut s2, rhoprime.as_ptr(), $l as u16);
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

                let mut public_key = vec![0u8; $pk_len];
                unsafe {
                    $pack_pk(public_key.as_mut_ptr(), rho.as_ptr(), &t1);
                }

                let mut tr = Zeroizing::new([0u8; TRBYTES]);
                shake256_into(tr.as_mut(), &[&public_key]);

                let mut secret_key = Zeroizing::new(vec![0u8; $sk_len]);
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
                let mut t0 = Polyveck::<$k>::default();
                let mut s1 = Polyvecl::<$l>::default();
                let mut s2 = Polyveck::<$k>::default();

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

                let mut mat = Zeroizing::new(vec![Polyvecl::<$l>::default(); $k]);
                unsafe {
                    $matrix_expand(mat.as_mut_ptr(), rho.as_ptr());
                    $vecl_ntt(&mut s1);
                    $veck_ntt(&mut s2);
                    $veck_ntt(&mut t0);
                }

                let mut sig = vec![0u8; $sig_len];
                let mut nonce = 0u16;

                loop {
                    let mut y = Polyvecl::<$l>::default();
                    let mut z;
                    let mut w1 = Polyveck::<$k>::default();
                    let mut w0 = Polyveck::<$k>::default();
                    let mut h = Polyveck::<$k>::default();
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

                    let mut w1_packed = Zeroizing::new(vec![0u8; $k * $polyw1]);
                    unsafe {
                        $veck_pack_w1(w1_packed.as_mut_ptr(), &w1);
                    }

                    let mut c_tilde = Zeroizing::new([0u8; $ctilde]);
                    shake256_into(c_tilde.as_mut(), &[mu.as_ref(), w1_packed.as_ref()]);
                    sig[..$ctilde].copy_from_slice(c_tilde.as_ref());

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
                    if (unsafe { $vecl_chknorm(&z, (($gamma1) - ($beta)) as i32) }) != 0 {
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
                    if (unsafe { $veck_chknorm(&w0, (($gamma2) - ($beta)) as i32) }) != 0 {
                        continue;
                    }

                    unsafe {
                        $veck_pointwise(&mut h, &cp, &t0);
                        $veck_invntt(&mut h);
                        $veck_reduce(&mut h);
                    }
                    if (unsafe { $veck_chknorm(&h, ($gamma2) as i32) }) != 0 {
                        continue;
                    }

                    let w0_before_add = w0.clone();
                    unsafe {
                        $veck_add(&mut w0, &w0_before_add, &h);
                    }
                    let hints = unsafe { $veck_make_hint(&mut h, &w0, &w1) };
                    if hints > $omega {
                        continue;
                    }

                    unsafe {
                        $pack_sig(sig.as_mut_ptr(), sig.as_ptr(), &z, &h);
                    }
                    return Ok(sig);
                }
            }

            #[allow(unsafe_code)]
            unsafe extern "C" {
                fn $matrix_expand(mat: *mut Polyvecl<$l>, rho: *const u8);
                fn $vecl_uniform_eta(v: *mut Polyvecl<$l>, seed: *const u8, nonce: u16);
                fn $veck_uniform_eta(v: *mut Polyveck<$k>, seed: *const u8, nonce: u16);
                fn $vecl_uniform_gamma1(v: *mut Polyvecl<$l>, seed: *const u8, nonce: u16);
                fn $vecl_ntt(v: *mut Polyvecl<$l>);
                fn $vecl_invntt(v: *mut Polyvecl<$l>);
                fn $vecl_pointwise(r: *mut Polyvecl<$l>, a: *const Poly, v: *const Polyvecl<$l>);
                fn $vecl_add(w: *mut Polyvecl<$l>, u: *const Polyvecl<$l>, v: *const Polyvecl<$l>);
                fn $vecl_reduce(v: *mut Polyvecl<$l>);
                fn $vecl_chknorm(v: *const Polyvecl<$l>, bound: i32) -> c_int;
                fn $veck_ntt(v: *mut Polyveck<$k>);
                fn $veck_invntt(v: *mut Polyveck<$k>);
                fn $veck_pointwise(r: *mut Polyveck<$k>, a: *const Poly, v: *const Polyveck<$k>);
                fn $veck_reduce(v: *mut Polyveck<$k>);
                fn $veck_chknorm(v: *const Polyveck<$k>, bound: i32) -> c_int;
                fn $veck_caddq(v: *mut Polyveck<$k>);
                fn $veck_add(w: *mut Polyveck<$k>, u: *const Polyveck<$k>, v: *const Polyveck<$k>);
                fn $veck_sub(w: *mut Polyveck<$k>, u: *const Polyveck<$k>, v: *const Polyveck<$k>);
                fn $veck_power2round(
                    v1: *mut Polyveck<$k>,
                    v0: *mut Polyveck<$k>,
                    v: *const Polyveck<$k>,
                );
                fn $veck_decompose(
                    v1: *mut Polyveck<$k>,
                    v0: *mut Polyveck<$k>,
                    v: *const Polyveck<$k>,
                );
                fn $veck_make_hint(
                    h: *mut Polyveck<$k>,
                    v0: *const Polyveck<$k>,
                    v1: *const Polyveck<$k>,
                ) -> u32;
                fn $veck_pack_w1(out: *mut u8, w1: *const Polyveck<$k>);
                fn $matrix_pointwise(
                    t: *mut Polyveck<$k>,
                    mat: *const Polyvecl<$l>,
                    v: *const Polyvecl<$l>,
                );
                fn $poly_challenge(c: *mut Poly, seed: *const u8);
                fn $poly_ntt(p: *mut Poly);
                fn $pack_pk(pk: *mut u8, rho: *const u8, t1: *const Polyveck<$k>);
                fn $pack_sk(
                    sk: *mut u8,
                    rho: *const u8,
                    tr: *const u8,
                    key: *const u8,
                    t0: *const Polyveck<$k>,
                    s1: *const Polyvecl<$l>,
                    s2: *const Polyveck<$k>,
                );
                fn $pack_sig(
                    sig: *mut u8,
                    c: *const u8,
                    z: *const Polyvecl<$l>,
                    h: *const Polyveck<$k>,
                );
                fn $unpack_sk(
                    rho: *mut u8,
                    tr: *mut u8,
                    key: *mut u8,
                    t0: *mut Polyveck<$k>,
                    s1: *mut Polyvecl<$l>,
                    s2: *mut Polyveck<$k>,
                    sk: *const u8,
                );
            }
        }
    };
}

mldsa_suite!(
    mldsa44,
    MlDsaSuite::MlDsa44,
    k = 4,
    l = 4,
    beta = 78,
    gamma1 = 1 << 17,
    gamma2 = (8_380_417 - 1) / 88,
    omega = 80,
    ctilde = 32,
    polyw1 = 192,
    pk_len = 1312,
    sk_len = 2560,
    sig_len = 2420,
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
    k = 6,
    l = 5,
    beta = 196,
    gamma1 = 1 << 19,
    gamma2 = (8_380_417 - 1) / 32,
    omega = 55,
    ctilde = 48,
    polyw1 = 128,
    pk_len = 1952,
    sk_len = 4032,
    sig_len = 3309,
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
    k = 8,
    l = 7,
    beta = 120,
    gamma1 = 1 << 19,
    gamma2 = (8_380_417 - 1) / 32,
    omega = 75,
    ctilde = 64,
    polyw1 = 128,
    pk_len = 2592,
    sk_len = 4896,
    sig_len = 4627,
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
