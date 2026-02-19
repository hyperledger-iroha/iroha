//! Transparent (untrusted-setup) Halo2-style proof system skeleton.
//!
//! This crate provides a from-scratch, transparent polynomial-commitment and
//! Inner-Product Argument (IPA) based opening scheme suitable as the backbone
//! for Halo2/PLONKish arithmetizations. It focuses on determinism and
//! portability, and avoids any trusted setup by deriving generators
//! deterministically from a transcript domain separation tag (DST).
//!
//! Notes
//! - This is a foundational, minimal implementation intended for integration
//!   and iteration. It implements deterministic transcripts (SHA3-256/512),
//!   polynomial commit/open with IPA proofs, and ships deterministic backends
//!   for Pasta/Pallas and BN254. An optional Goldilocks backend remains
//!   available for compatibility testing.
//! - Cryptographic security depends on the chosen backend curve/field. Both
//!   backends derive generators transparently from the shared DST so proofs are
//!   reproducible across hosts. Additional backends can be added in the future
//!   as deterministic implementations mature.
//! - API is designed to be no-std-friendly in the future, but currently
//!   targets `std` for simplicity.

#![deny(missing_docs)]
#![deny(unsafe_code)]

pub mod backend;
pub mod confidential;
pub mod envelope;
mod errors;
mod field;
mod group;
mod hash;
mod ipa;
mod norito_types;
mod params;
mod poly;
pub mod poseidon;
mod transcript;

// Re-exports for the default (Pallas) backend.
#[cfg(feature = "goldilocks_backend")]
pub use backend::goldilocks::{
    Group as GoldilocksGroup, IpaProof as GoldilocksIpaProof, IpaProver as GoldilocksIpaProver,
    IpaVerifier as GoldilocksIpaVerifier, Params as GoldilocksParams,
    Polynomial as GoldilocksPolynomial, Scalar as GoldilocksScalar,
};
pub use backend::{
    IpaBackend, IpaGroup, IpaScalar, bn254,
    bn254::{
        GroupElem as Bn254Group, IpaProof as Bn254IpaProof, IpaProver as Bn254IpaProver,
        IpaVerifier as Bn254IpaVerifier, Params as Bn254Params, Polynomial as Bn254Polynomial,
        Scalar as Bn254Scalar,
    },
    pallas,
    pallas::{
        Group as GroupElem, IpaProof, IpaProver, IpaVerifier, Params, Polynomial,
        Scalar as PrimeField64,
    },
};
pub use envelope::{
    CURVE_PASTA, ENVELOPE_VERSION, EnvelopeError, FLAG_LOOKUPS, Halo2ProofEnvelope,
    Halo2ProofEnvelopeHeader, PCS_IPA, PUBLIC_INPUT_STRIDE, TRANSCRIPT_BLAKE2B,
};
pub use errors::Error;
pub use norito_types::{IpaParams, IpaProofData, OpenVerifyEnvelope, PolyOpenPublic, ZkCurveId};
pub use transcript::Transcript;

/// Crate constants and domain separation tags.
pub mod constants {
    /// Domain Separation Tag for generator derivation and transcript usage.
    pub const DST: &str = "IROHA-ZK-HALO2-IPA-v1";
}

// Test module (private)
#[cfg(test)]
mod tests;

pub mod norito_helpers {
    //! Conversions between internal types and Norito wire types.
    use std::sync::Arc;

    use super::*;
    use crate::{
        backend::{IpaBackend, pallas::PallasBackend},
        errors::Error,
    };

    /// Encode backend parameters into the Norito wire representation.
    pub fn params_to_wire<B: IpaBackend>(params: &crate::params::Params<B>) -> IpaParams {
        IpaParams {
            version: 1,
            curve_id: B::CURVE_ID.as_u16(),
            n: params.n() as u32,
            g: params.g().iter().map(|g| g.to_bytes()).collect(),
            h: params.h().iter().map(|h| h.to_bytes()).collect(),
            u: params.u().to_bytes(),
        }
    }

    /// Decode parameters for a specific backend.
    pub fn params_from_wire<B: IpaBackend + 'static>(
        w: &IpaParams,
    ) -> Result<Arc<crate::params::Params<B>>, Error> {
        crate::params::params_from_wire_backend::<B>(w)
    }

    /// Register a parameter set so that subsequent decodes recognize it.
    pub fn register_params_from_wire<B: IpaBackend + 'static>(
        w: &IpaParams,
    ) -> Result<Arc<crate::params::Params<B>>, Error> {
        crate::params::register_params_from_wire::<B>(w)
    }

    /// Encode an IPA proof for transport.
    pub fn proof_to_wire<B: IpaBackend>(proof: &crate::ipa::IpaProof<B>) -> IpaProofData {
        IpaProofData {
            version: 1,
            l: proof.l_vec.iter().map(|g| g.to_bytes()).collect(),
            r: proof.r_vec.iter().map(|g| g.to_bytes()).collect(),
            a_final: proof.a_final.to_bytes(),
            b_final: proof.b_final.to_bytes(),
        }
    }

    /// Decode an IPA proof for a specific backend.
    pub fn proof_from_wire<B: IpaBackend>(
        w: &IpaProofData,
    ) -> Result<crate::ipa::IpaProof<B>, Error> {
        proof_from_wire_backend::<B>(w)
    }

    fn proof_from_wire_backend<B: IpaBackend>(
        w: &IpaProofData,
    ) -> Result<crate::ipa::IpaProof<B>, Error> {
        if w.version != 1 {
            return Err(Error::VerificationFailed);
        }
        let l_vec =
            w.l.iter()
                .map(B::Group::from_bytes)
                .collect::<Result<Vec<_>, _>>()?;
        let r_vec =
            w.r.iter()
                .map(B::Group::from_bytes)
                .collect::<Result<Vec<_>, _>>()?;
        let a_final = B::Scalar::from_bytes(&w.a_final)?;
        let b_final = B::Scalar::from_bytes(&w.b_final)?;
        Ok(crate::ipa::IpaProof {
            l_vec,
            r_vec,
            a_final,
            b_final,
        })
    }

    /// Encode the public portion of a polynomial opening.
    pub fn poly_open_public<B: IpaBackend>(
        n: usize,
        z: B::Scalar,
        t: B::Scalar,
        p_g: B::Group,
    ) -> PolyOpenPublic {
        PolyOpenPublic {
            version: 1,
            curve_id: B::CURVE_ID.as_u16(),
            n: n as u32,
            z: z.to_bytes(),
            t: t.to_bytes(),
            p_g: p_g.to_bytes(),
        }
    }

    /// Decode and dispatch a polynomial opening envelope according to its backend.
    pub fn decode_envelope(env: &OpenVerifyEnvelope) -> Result<DecodedEnvelope, Error> {
        if env.params.n != env.public.n {
            return Err(Error::VerificationFailed);
        }
        if env.params.curve_id != env.public.curve_id {
            return Err(Error::VerificationFailed);
        }
        match ZkCurveId::from_u16(env.params.curve_id) {
            ZkCurveId::Pallas => {
                let params = params_from_wire::<PallasBackend>(&env.params)?;
                let proof = proof_from_wire_backend::<PallasBackend>(&env.proof)?;
                let z = <PallasBackend as IpaBackend>::Scalar::from_bytes(&env.public.z)?;
                let t = <PallasBackend as IpaBackend>::Scalar::from_bytes(&env.public.t)?;
                let p_g = <PallasBackend as IpaBackend>::Group::from_bytes(&env.public.p_g)?;
                Ok(DecodedEnvelope::Pallas {
                    params,
                    proof: Box::new(proof),
                    z,
                    t,
                    p_g,
                })
            }
            ZkCurveId::Goldilocks => {
                #[cfg(feature = "goldilocks_backend")]
                {
                    let params =
                        params_from_wire::<backend::goldilocks::GoldilocksBackend>(&env.params)?;
                    let proof = proof_from_wire_backend::<backend::goldilocks::GoldilocksBackend>(
                        &env.proof,
                    )?;
                    let z =
                        <backend::goldilocks::GoldilocksBackend as IpaBackend>::Scalar::from_bytes(
                            &env.public.z,
                        )?;
                    let t =
                        <backend::goldilocks::GoldilocksBackend as IpaBackend>::Scalar::from_bytes(
                            &env.public.t,
                        )?;
                    let p_g =
                        <backend::goldilocks::GoldilocksBackend as IpaBackend>::Group::from_bytes(
                            &env.public.p_g,
                        )?;
                    Ok(DecodedEnvelope::Goldilocks {
                        params,
                        proof: Box::new(proof),
                        z,
                        t,
                        p_g,
                    })
                }
                #[cfg(not(feature = "goldilocks_backend"))]
                {
                    Err(Error::UnsupportedBackend {
                        backend: ZkCurveId::Goldilocks,
                    })
                }
            }
            ZkCurveId::Bn254 => {
                let params = params_from_wire::<backend::bn254::Bn254Backend>(&env.params)?;
                let proof = proof_from_wire_backend::<backend::bn254::Bn254Backend>(&env.proof)?;
                let z = <backend::bn254::Bn254Backend as IpaBackend>::Scalar::from_bytes(
                    &env.public.z,
                )?;
                let t = <backend::bn254::Bn254Backend as IpaBackend>::Scalar::from_bytes(
                    &env.public.t,
                )?;
                let p_g = <backend::bn254::Bn254Backend as IpaBackend>::Group::from_bytes(
                    &env.public.p_g,
                )?;
                Ok(DecodedEnvelope::Bn254 {
                    params,
                    proof: Box::new(proof),
                    z,
                    t,
                    p_g,
                })
            }
            _ => Err(Error::VerificationFailed),
        }
    }

    /// Convenience wrapper for decoding group bytes under the Pallas backend.
    pub fn group_from_bytes(bytes: &[u8; 32]) -> Result<GroupElem, Error> {
        <PallasBackend as IpaBackend>::Group::from_bytes(bytes)
    }

    /// Envelope decoded into backend-specific components.
    #[derive(Debug)]
    pub enum DecodedEnvelope {
        /// Pallas backend contents.
        Pallas {
            /// Parameters (generator set).
            params: Arc<crate::params::Params<PallasBackend>>,
            /// IPA proof body.
            proof: Box<crate::ipa::IpaProof<PallasBackend>>,
            /// Evaluation point.
            z: backend::pallas::Scalar,
            /// Claimed evaluation.
            t: backend::pallas::Scalar,
            /// Commitment to coefficients.
            p_g: backend::pallas::Group,
        },
        /// Goldilocks backend contents.
        #[cfg(feature = "goldilocks_backend")]
        Goldilocks {
            /// Parameters (generator set).
            params: Arc<crate::params::Params<backend::goldilocks::GoldilocksBackend>>,
            /// IPA proof body.
            proof: Box<crate::ipa::IpaProof<backend::goldilocks::GoldilocksBackend>>,
            /// Evaluation point.
            z: backend::goldilocks::Scalar,
            /// Claimed evaluation.
            t: backend::goldilocks::Scalar,
            /// Commitment to coefficients.
            p_g: backend::goldilocks::Group,
        },
        #[cfg(not(feature = "goldilocks_backend"))]
        /// Goldilocks backend placeholder when the backend is not compiled in.
        Goldilocks,
        /// BN254 backend contents.
        Bn254 {
            /// Parameters (generator set).
            params: Arc<crate::params::Params<backend::bn254::Bn254Backend>>,
            /// IPA proof body.
            proof: Box<crate::ipa::IpaProof<backend::bn254::Bn254Backend>>,
            /// Evaluation point.
            z: backend::bn254::Scalar,
            /// Claimed evaluation.
            t: backend::bn254::Scalar,
            /// Commitment to coefficients.
            p_g: backend::bn254::GroupElem,
        },
    }
}

/// Batch verification helpers for OpenVerify envelopes.
pub mod batch {
    use core::num::NonZeroUsize;

    use norito_helpers::DecodedEnvelope;
    #[cfg(feature = "parallel")]
    use rayon::prelude::*;
    #[cfg(feature = "parallel")]
    use std::{
        collections::HashMap,
        sync::{Arc, Mutex, OnceLock},
    };

    use super::*;

    #[cfg(feature = "parallel")]
    static LIMITED_POOL_CACHE: OnceLock<Mutex<HashMap<usize, Arc<rayon::ThreadPool>>>> =
        OnceLock::new();
    #[cfg(feature = "parallel")]
    const LIMITED_POOL_CACHE_MAX_ENTRIES: usize = 16;

    #[cfg(feature = "parallel")]
    fn limited_pool(limit: NonZeroUsize) -> Option<Arc<rayon::ThreadPool>> {
        let key = limit.get();
        let cache = LIMITED_POOL_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
        {
            let guard = cache.lock().ok()?;
            if let Some(pool) = guard.get(&key) {
                return Some(pool.clone());
            }
        }
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(key)
            .build()
            .ok()?;
        let pool = Arc::new(pool);
        if let Ok(mut guard) = cache.lock() {
            if !guard.contains_key(&key)
                && guard.len() >= LIMITED_POOL_CACHE_MAX_ENTRIES
                && let Some(evict_key) = guard.keys().copied().max()
            {
                guard.remove(&evict_key);
            }
            guard.entry(key).or_insert_with(|| pool.clone());
        }
        Some(pool)
    }

    /// Execution strategy controls how batch verification work is scheduled.
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum Parallelism {
        /// Run verification sequentially on the current thread.
        Sequential,
        /// Use the ambient rayon thread-pool (when the `parallel` feature is enabled).
        Auto,
        /// Use at most the provided number of rayon worker threads (>= 1).
        Limited(NonZeroUsize),
    }

    /// Options controlling the batch verifier behaviour.
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub struct BatchOptions {
        /// Parallelism strategy for the batch verifier.
        pub parallelism: Parallelism,
    }

    impl Default for BatchOptions {
        fn default() -> Self {
            #[cfg(feature = "parallel")]
            {
                Self {
                    parallelism: Parallelism::Auto,
                }
            }
            #[cfg(not(feature = "parallel"))]
            {
                Self {
                    parallelism: Parallelism::Sequential,
                }
            }
        }
    }

    impl BatchOptions {
        /// Run batch verification sequentially.
        pub const fn sequential() -> Self {
            Self {
                parallelism: Parallelism::Sequential,
            }
        }

        /// Run batch verification using the ambient rayon pool when available.
        pub const fn auto() -> Self {
            Self {
                parallelism: Parallelism::Auto,
            }
        }

        /// Limit the number of rayon threads used during verification.
        pub const fn limited(max_threads: NonZeroUsize) -> Self {
            Self {
                parallelism: Parallelism::Limited(max_threads),
            }
        }
    }

    fn verify_single(env: &OpenVerifyEnvelope) -> Result<bool, Error> {
        let decoded = norito_helpers::decode_envelope(env)?;
        let mut transcript = Transcript::new(&env.transcript_label);
        let result = match decoded {
            DecodedEnvelope::Pallas {
                params,
                proof,
                z,
                t,
                p_g,
            } => backend::pallas::Polynomial::verify_open(
                params.as_ref(),
                &mut transcript,
                z,
                p_g,
                t,
                proof.as_ref(),
            ),
            DecodedEnvelope::Bn254 {
                params,
                proof,
                z,
                t,
                p_g,
            } => backend::bn254::Polynomial::verify_open(
                params.as_ref(),
                &mut transcript,
                z,
                p_g,
                t,
                proof.as_ref(),
            ),
            #[cfg(feature = "goldilocks_backend")]
            DecodedEnvelope::Goldilocks {
                params,
                proof,
                z,
                t,
                p_g,
            } => backend::goldilocks::Polynomial::verify_open(
                params.as_ref(),
                &mut transcript,
                z,
                p_g,
                t,
                proof.as_ref(),
            ),
            #[cfg(not(feature = "goldilocks_backend"))]
            DecodedEnvelope::Goldilocks => {
                return Err(Error::UnsupportedBackend {
                    backend: ZkCurveId::Goldilocks,
                });
            }
        };
        match result {
            Ok(()) => Ok(true),
            Err(e) => {
                if matches!(e, Error::VerificationFailed) {
                    Ok(false)
                } else {
                    Err(e)
                }
            }
        }
    }

    fn verify_sequential(envelopes: &[OpenVerifyEnvelope]) -> Vec<Result<bool, Error>> {
        envelopes.iter().map(verify_single).collect()
    }

    /// Verify multiple `OpenVerifyEnvelope`s and return per-envelope results.
    pub fn verify_open_batch(envelopes: &[OpenVerifyEnvelope]) -> Vec<Result<bool, Error>> {
        verify_open_batch_with_options(envelopes, &BatchOptions::default())
    }

    /// Verify multiple `OpenVerifyEnvelope`s using the supplied batch options.
    pub fn verify_open_batch_with_options(
        envelopes: &[OpenVerifyEnvelope],
        options: &BatchOptions,
    ) -> Vec<Result<bool, Error>> {
        if envelopes.is_empty() {
            return Vec::new();
        }
        match options.parallelism {
            Parallelism::Sequential => verify_sequential(envelopes),
            Parallelism::Auto => {
                #[cfg(feature = "parallel")]
                {
                    envelopes.par_iter().map(verify_single).collect()
                }
                #[cfg(not(feature = "parallel"))]
                {
                    verify_sequential(envelopes)
                }
            }
            Parallelism::Limited(limit) => {
                #[cfg(feature = "parallel")]
                {
                    if limit.get() <= 1 {
                        return verify_sequential(envelopes);
                    }
                    limited_pool(limit).map_or_else(
                        || verify_sequential(envelopes),
                        |pool| pool.install(|| envelopes.par_iter().map(verify_single).collect()),
                    )
                }
                #[cfg(not(feature = "parallel"))]
                {
                    let _ = limit;
                    verify_sequential(envelopes)
                }
            }
        }
    }

    #[cfg(all(test, feature = "parallel"))]
    mod tests {
        use super::*;

        #[test]
        fn limited_pool_cache_is_bounded() {
            let cache = LIMITED_POOL_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
            cache.lock().expect("cache lock").clear();
            for threads in 1..=(LIMITED_POOL_CACHE_MAX_ENTRIES + 8) {
                let limit = NonZeroUsize::new(threads).expect("non-zero thread count");
                let _ = limited_pool(limit);
            }
            let len = cache.lock().expect("cache lock").len();
            assert!(
                len <= LIMITED_POOL_CACHE_MAX_ENTRIES,
                "cache size {len} should be <= {}",
                LIMITED_POOL_CACHE_MAX_ENTRIES
            );
        }
    }
}
