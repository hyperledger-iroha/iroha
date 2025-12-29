mod curve;
mod engine;
mod fq;
mod fq12;
mod fq2;
mod fq6;
mod fr;

#[cfg(feature = "asm")]
mod assembly;

pub use curve::*;
pub use engine::*;
pub use fq::*;
pub use fq12::*;
pub use fq2::*;
pub use fq6::*;
pub use fr::*;

#[cfg(test)]
mod test {
    use super::G1 as Bn256Point;
    use group::GroupEncoding;
    use pasta_curves::arithmetic::CurveExt;
    use rand_core::{RngCore, SeedableRng};

    #[test]
    fn test_consistent_hash_to_curve() {
        // The goal of this test is to generate test vectors to ensure that the ASM implementation
        // matches the rust implementation.
        let num_vecs = 10;

        // Test vectors generated with rust implementation.
        let expected_results = [
            "e0c5a6834e0329b4f8bdc91144b3e687ac9d810a8e899415267db9cfbf61e91e",
            "7052a20bee99cbe054fdd8b2e336db3ed3e9a265229e44ab8197c5eabdef2b0b",
            "2f058acc133957074ac79e9b9b1867a0cf3d13df7aa7de7f48e9a6be7d96aa6d",
            "b2ff44a25693b811f35e33feb3e99ad9ba0d06425a3ffd5e79cef63d20143314",
            "ab2f6d71d2fde51546d8a5782aa9f707e585b84644470f0c876784dbebd30c55",
            "6a4e0e30f37a8d1b92b8cf08df3735a36b4937ee455a9dc5f9283a13530db144",
            "f1c69be8c5f5f9e28b0e9f76ab77651a7dcaaae371fbba66450cbcee0ed5b16b",
            "e86267c2e3355d7a6f664a0ea71374406337d452a3f9a294a0594df53c08df21",
            "03cf55ca983ecd8a2e2baae18d979d97d688a978d829701c66a14d7c4da58e62",
            "5302c2cfe3c909e9378d08c951bb33d0813818a1baf734379aac8aaa47f38f0d",
        ];

        let mut seeded_rng = rand_chacha::ChaChaRng::seed_from_u64(0u64);
        let uniform_bytes = std::iter::from_fn(|| {
            let mut bytes = [0u8; 32];
            seeded_rng.fill_bytes(&mut bytes);
            Some(bytes)
        })
        .take(num_vecs)
        .collect::<Vec<_>>();
        let hash = Bn256Point::hash_to_curve("from_uniform_bytes");
        for i in 0..num_vecs {
            let p = hash(&uniform_bytes[i]);
            let expected_result = hex::decode(expected_results[i]).unwrap();
            assert_eq!(
                p.to_bytes().as_ref(),
                &expected_result[..],
                "hash_to_curve_print failed, expected: {}, got: {}",
                expected_results[i],
                hex::encode(p.to_bytes().as_ref())
            );
        }
    }
}
