//! Ownership, redaction and erasure of BFV secret-key coefficients.

use super::BfvSecretKey;
use std::fmt;
use subtle::ConstantTimeEq;
use zeroize::{Zeroize, ZeroizeOnDrop};

impl BfvSecretKey {
    /// Borrow the private ternary polynomial for an explicit key-owner operation.
    ///
    /// The slice cannot transfer or mutate this owner's allocation. Any copy
    /// made by the caller requires its own clearing lifetime.
    #[must_use]
    pub fn coefficients(&self) -> &[u64] {
        &self.s
    }
}

impl fmt::Debug for BfvSecretKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BfvSecretKey")
            .field("coefficients", &"[REDACTED]")
            .finish()
    }
}

impl PartialEq for BfvSecretKey {
    fn eq(&self, other: &Self) -> bool {
        // Geometry is public; comparison of equally sized coefficient arrays
        // performs fixed work through subtle's slice implementation.
        bool::from(self.s.ct_eq(&other.s))
    }
}

impl Eq for BfvSecretKey {}

impl Zeroize for BfvSecretKey {
    fn zeroize(&mut self) {
        self.s.as_mut_slice().zeroize();
        #[cfg(test)]
        tests::observe_erasure(&self.s);
        // Vec::zeroize also clears unused capacity before deallocation.
        self.s.zeroize();
    }
}

impl Drop for BfvSecretKey {
    fn drop(&mut self) {
        self.zeroize();
    }
}

impl ZeroizeOnDrop for BfvSecretKey {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fhe_bfv::{
        BfvParameters, apply_galois_automorphism_poly,
        full_bootstrap_sample_extraction_switch_target_secret,
    };

    thread_local! {
        static CLEARED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    }

    pub(super) fn observe_erasure(coefficients: &[u64]) {
        assert!(coefficients.iter().all(|&value| value == 0));
        CLEARED.with(|cleared| cleared.set(cleared.get() + coefficients.len()));
    }

    fn cleared() -> usize {
        CLEARED.with(std::cell::Cell::get)
    }

    fn switch_fixture() -> (BfvParameters, BfvSecretKey) {
        let params = BfvParameters {
            polynomial_degree: 8,
            ciphertext_modulus: 16_777_216,
            plaintext_modulus: 256,
            decomposition_base_log: 12,
        };
        let minus_one = params.ciphertext_modulus - 1;
        let key = BfvSecretKey {
            s: vec![0, 1, minus_one, 1, 0, minus_one, 1, 1],
        };
        (params, key)
    }

    fn require_clearing_owner<T: ZeroizeOnDrop>(_: &T) {}

    #[test]
    fn automorphed_secret_retains_clearing_owner_and_negacyclic_signs() {
        let (params, key) = switch_fixture();
        let mut transformed = apply_galois_automorphism_poly(&params, key.coefficients(), 3)
            .expect("valid odd automorphism");
        require_clearing_owner(&transformed);
        assert_eq!(
            transformed.as_slice(),
            &[
                0,
                params.ciphertext_modulus - 1,
                1,
                1,
                0,
                1,
                params.ciphertext_modulus - 1,
                1
            ]
        );
        transformed.zeroize();
        assert!(transformed.is_empty());
        assert!(apply_galois_automorphism_poly(&params, key.coefficients(), 2).is_err());
        assert_eq!(key.coefficients()[2], params.ciphertext_modulus - 1);
    }

    #[test]
    fn sample_switch_target_retains_clearing_owner_and_validates_index() {
        let (params, key) = switch_fixture();
        let mut target = full_bootstrap_sample_extraction_switch_target_secret(&params, &key, 2)
            .expect("valid secret coefficient");
        require_clearing_owner(&target);
        assert_eq!(
            target.as_slice(),
            &[params.ciphertext_modulus - 1, 0, 0, 0, 0, 0, 0, 0]
        );
        target.zeroize();
        assert!(target.is_empty());
        assert!(full_bootstrap_sample_extraction_switch_target_secret(&params, &key, 8).is_err());
        assert_eq!(key.coefficients()[2], params.ciphertext_modulus - 1);
    }

    #[test]
    fn redacted_borrowed_coefficients_and_independent_clones() {
        let key = BfvSecretKey {
            s: vec![0, 1, 65_535, 17],
        };
        let mut other = key.clone();
        assert_eq!(key, other);
        assert_eq!(key.coefficients(), &[0, 1, 65_535, 17]);
        assert!(format!("{key:?}").contains("REDACTED"));
        assert!(!format!("{key:?}").contains("65535"));
        other.s[3] = 18;
        assert_ne!(key, other);
        assert_ne!(
            key,
            BfvSecretKey {
                s: vec![0, 1, 65_535]
            }
        );
        let before = cleared();
        other.zeroize();
        assert!(other.coefficients().is_empty());
        assert_eq!(cleared() - before, 4);
        assert_eq!(key.coefficients()[3], 17);
    }

    #[test]
    fn drop_error_and_unwind_clear_live_coefficients() {
        fn fail() -> Result<(), ()> {
            let _owner = BfvSecretKey { s: vec![1, 2, 3] };
            Err(())?;
            Ok(())
        }
        let before = cleared();
        drop(BfvSecretKey {
            s: vec![1, 2, 3, 4],
        });
        assert_eq!(cleared() - before, 4);
        assert!(fail().is_err());
        assert_eq!(cleared() - before, 7);
        let unwind = std::panic::catch_unwind(|| {
            let _owner = BfvSecretKey { s: vec![5, 6] };
            panic!("test-only secret owner unwind");
        });
        assert!(unwind.is_err());
        assert_eq!(cleared() - before, 9);
    }

    #[test]
    fn secret_key_codec_roundtrip_retains_clearing_owner() {
        let key = BfvSecretKey { s: vec![0, 1, 256] };
        let bytes = zeroize::Zeroizing::new(norito::encode_canonical(&key).unwrap());
        let decoded = norito::decode_from_bytes::<BfvSecretKey>(&bytes).unwrap();
        assert_eq!(decoded, key);
        let before = cleared();
        drop(decoded);
        assert_eq!(cleared() - before, 3);
    }

    #[cfg(feature = "json")]
    #[test]
    fn secret_key_json_roundtrip_retains_clearing_owner() {
        let key = BfvSecretKey { s: vec![0, 1, 256] };
        let bytes = zeroize::Zeroizing::new(norito::json::to_json(&key).unwrap());
        let decoded: BfvSecretKey = norito::json::from_str(&bytes).unwrap();
        assert_eq!(decoded, key);
        let before = cleared();
        drop(decoded);
        assert_eq!(cleared() - before, 3);
    }
}
