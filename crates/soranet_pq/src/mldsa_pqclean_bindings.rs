// @generated from the allowlisted PQClean headers named below.
//
// Source: pqcrypto-mldsa 0.1.2 / PQClean clean ML-DSA headers.
// The Cargo dependency is pinned exactly to that release. Regenerate this file
// before changing the backend version; never hand-edit polynomial dimensions
// or foreign function signatures.
//
// Header SHA-256 values (params.h, poly.h, polyvec.h, packing.h):
// ML-DSA-44:
//   0210251cea61d26e49b2dad16c4ed86d65474fbffa54c61af7a22c677ddd3cd2
//   e6cbe386564946336452ef0694583bc9d8081ed2d81310622644bb9e36560f14
//   7fc533b6536819f5d52b31c7a4fa29d15d1f0804138b4ea82ff42bd6a8aaffdf
//   55eded057c9d78299e169ff5329915dc3beba291634186ca69f445c71167b5ea
// ML-DSA-65:
//   9a8cbfeb0f0573d8e0bfe1bd14ffaf0e342ba1adf48d09b49f9d5f4b7eab6928
//   a71de330d5a896f89094acbdfd96546b91ab10330d0cf4229d85a653c95013c1
//   c0cbfb2c9082f63777fd1f8feeb187faf7b6b6153e8e5454243340430dcc4069
//   2242c5b64ec063c6be015ade9d40df27d1088d3a87ad30ac8cb1bd3cb2434eb5
// ML-DSA-87:
//   26683f8cc27c38d1ac6daa21523207d515be060ea1f8aaa1c9fd160035da1780
//   f5404348be6c45d17ed702e5c7c3bf4fb4e18490a92afcfbf6aff9848b1ccc27
//   4e112d4673a41b889dcf9dff64cc02b583938c2947f5391136b1e32c4cd6c7e7
//   5435128d7795935147859d02adf96c58601c83258f9eb51c71a0f05e7011fa64

macro_rules! define_pqclean_mldsa_bindings {
    (
        $module:ident,
        library = $library:literal,
        n = $n:expr,
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
        pub(crate) mod $module {
            use core::{
                array,
                ffi::{c_int, c_uint},
            };

            use zeroize::{Zeroize, ZeroizeOnDrop};

            pub(crate) const K: usize = $k;
            pub(crate) const K_DOMAIN_BYTE: u8 = $k;
            pub(crate) const L_DOMAIN_BYTE: u8 = $l;
            pub(crate) const L_NONCE: u16 = $l;
            pub(crate) const BETA: i32 = $beta;
            pub(crate) const GAMMA1: i32 = $gamma1;
            pub(crate) const GAMMA2: i32 = $gamma2;
            pub(crate) const OMEGA: c_uint = $omega;
            pub(crate) const CTILDEBYTES: usize = $ctilde;
            pub(crate) const POLYW1_PACKEDBYTES: usize = $polyw1;
            pub(crate) const PUBLIC_KEY_BYTES: usize = $pk_len;
            pub(crate) const SECRET_KEY_BYTES: usize = $sk_len;
            pub(crate) const SIGNATURE_BYTES: usize = $sig_len;

            #[repr(C)]
            #[derive(Clone, PartialEq, Eq, Zeroize, ZeroizeOnDrop)]
            pub(crate) struct Poly {
                coeffs: [i32; $n],
            }

            impl Default for Poly {
                fn default() -> Self {
                    Self { coeffs: [0; $n] }
                }
            }

            #[repr(C)]
            #[derive(Clone, Zeroize, ZeroizeOnDrop)]
            pub(crate) struct Polyvecl {
                vec: [Poly; $l],
            }

            impl Default for Polyvecl {
                fn default() -> Self {
                    Self {
                        vec: array::from_fn(|_| Poly::default()),
                    }
                }
            }

            #[repr(C)]
            #[derive(Clone, PartialEq, Eq, Zeroize, ZeroizeOnDrop)]
            pub(crate) struct Polyveck {
                vec: [Poly; $k],
            }

            impl Default for Polyveck {
                fn default() -> Self {
                    Self {
                        vec: array::from_fn(|_| Poly::default()),
                    }
                }
            }

            const _: [(); core::mem::size_of::<Poly>()] = [(); $n * core::mem::size_of::<i32>()];
            const _: [(); core::mem::align_of::<Poly>()] = [(); core::mem::align_of::<i32>()];
            const _: [(); core::mem::size_of::<Polyvecl>()] =
                [(); $l * core::mem::size_of::<Poly>()];
            const _: [(); core::mem::align_of::<Polyvecl>()] = [(); core::mem::align_of::<Poly>()];
            const _: [(); core::mem::size_of::<Polyveck>()] =
                [(); $k * core::mem::size_of::<Poly>()];
            const _: [(); core::mem::align_of::<Polyveck>()] = [(); core::mem::align_of::<Poly>()];

            #[link(name = $library)]
            unsafe extern "C" {
                pub(crate) fn $matrix_expand(mat: *mut Polyvecl, rho: *const u8);
                pub(crate) fn $vecl_uniform_eta(v: *mut Polyvecl, seed: *const u8, nonce: u16);
                pub(crate) fn $veck_uniform_eta(v: *mut Polyveck, seed: *const u8, nonce: u16);
                pub(crate) fn $vecl_uniform_gamma1(v: *mut Polyvecl, seed: *const u8, nonce: u16);
                pub(crate) fn $vecl_ntt(v: *mut Polyvecl);
                pub(crate) fn $vecl_invntt(v: *mut Polyvecl);
                pub(crate) fn $vecl_pointwise(r: *mut Polyvecl, a: *const Poly, v: *const Polyvecl);
                pub(crate) fn $vecl_add(w: *mut Polyvecl, u: *const Polyvecl, v: *const Polyvecl);
                pub(crate) fn $vecl_reduce(v: *mut Polyvecl);
                pub(crate) fn $vecl_chknorm(v: *const Polyvecl, bound: i32) -> c_int;
                pub(crate) fn $veck_ntt(v: *mut Polyveck);
                pub(crate) fn $veck_invntt(v: *mut Polyveck);
                pub(crate) fn $veck_pointwise(r: *mut Polyveck, a: *const Poly, v: *const Polyveck);
                pub(crate) fn $veck_reduce(v: *mut Polyveck);
                pub(crate) fn $veck_chknorm(v: *const Polyveck, bound: i32) -> c_int;
                pub(crate) fn $veck_caddq(v: *mut Polyveck);
                pub(crate) fn $veck_add(w: *mut Polyveck, u: *const Polyveck, v: *const Polyveck);
                pub(crate) fn $veck_sub(w: *mut Polyveck, u: *const Polyveck, v: *const Polyveck);
                pub(crate) fn $veck_power2round(
                    v1: *mut Polyveck,
                    v0: *mut Polyveck,
                    v: *const Polyveck,
                );
                pub(crate) fn $veck_decompose(
                    v1: *mut Polyveck,
                    v0: *mut Polyveck,
                    v: *const Polyveck,
                );
                pub(crate) fn $veck_make_hint(
                    h: *mut Polyveck,
                    v0: *const Polyveck,
                    v1: *const Polyveck,
                ) -> c_uint;
                pub(crate) fn $veck_pack_w1(out: *mut u8, w1: *const Polyveck);
                pub(crate) fn $matrix_pointwise(
                    t: *mut Polyveck,
                    mat: *const Polyvecl,
                    v: *const Polyvecl,
                );
                pub(crate) fn $poly_challenge(c: *mut Poly, seed: *const u8);
                pub(crate) fn $poly_ntt(p: *mut Poly);
                pub(crate) fn $pack_pk(pk: *mut u8, rho: *const u8, t1: *const Polyveck);
                pub(crate) fn $pack_sk(
                    sk: *mut u8,
                    rho: *const u8,
                    tr: *const u8,
                    key: *const u8,
                    t0: *const Polyveck,
                    s1: *const Polyvecl,
                    s2: *const Polyveck,
                );
                pub(crate) fn $pack_sig(
                    sig: *mut u8,
                    c: *const u8,
                    z: *const Polyvecl,
                    h: *const Polyveck,
                );
                pub(crate) fn $unpack_sk(
                    rho: *mut u8,
                    tr: *mut u8,
                    key: *mut u8,
                    t0: *mut Polyveck,
                    s1: *mut Polyvecl,
                    s2: *mut Polyveck,
                    sk: *const u8,
                );
            }
        }
    };
}

define_pqclean_mldsa_bindings!(
    mldsa44,
    library = "ml-dsa-44_clean",
    n = 256,
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

define_pqclean_mldsa_bindings!(
    mldsa65,
    library = "ml-dsa-65_clean",
    n = 256,
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

define_pqclean_mldsa_bindings!(
    mldsa87,
    library = "ml-dsa-87_clean",
    n = 256,
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
