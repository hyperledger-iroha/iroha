//! Clearing lifetime for the private affine policy derived by a resolver.

use super::BfvAffineCircuit;
use std::ops::Deref;
use zeroize::Zeroize;

pub(super) struct SecretAffineCircuit(pub(super) BfvAffineCircuit);

impl SecretAffineCircuit {
    pub(super) fn with_capacity(outputs: usize) -> Self {
        Self(BfvAffineCircuit {
            weights: Vec::with_capacity(outputs),
            bias: Vec::with_capacity(outputs),
        })
    }
}

impl Deref for SecretAffineCircuit {
    type Target = BfvAffineCircuit;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Drop for SecretAffineCircuit {
    fn drop(&mut self) {
        for row in &mut self.0.weights {
            row.as_mut_slice().zeroize();
        }
        self.0.bias.as_mut_slice().zeroize();
        #[cfg(test)]
        tests::observe_erasure(&self.0);
        // Clearing Vec owners also covers spare coefficient capacity.
        self.0.weights.zeroize();
        self.0.bias.zeroize();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    thread_local! {
        static CLEARED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    }

    pub(super) fn observe_erasure(circuit: &BfvAffineCircuit) {
        let values = circuit.weights.iter().flatten().chain(&circuit.bias);
        let mut count = 0;
        for value in values {
            assert_eq!(*value, 0);
            count += 1;
        }
        CLEARED.with(|cleared| cleared.set(cleared.get() + count));
    }

    fn circuit() -> SecretAffineCircuit {
        let mut owner = SecretAffineCircuit::with_capacity(1);
        owner.0.weights.push(vec![13, 17]);
        owner.0.bias.push(23);
        owner
    }

    #[test]
    fn private_circuit_clears_on_drop_validation_error_and_unwind() {
        let before = CLEARED.with(std::cell::Cell::get);
        drop(circuit());
        assert_eq!(CLEARED.with(std::cell::Cell::get) - before, 3);
        let invalid = || {
            let owner = circuit();
            owner.validate(&crate::ram_lfe_bfv_parameters_v1(), 3)?;
            Ok::<_, crate::BfvError>(())
        };
        assert!(invalid().is_err());
        assert_eq!(CLEARED.with(std::cell::Cell::get) - before, 6);
        assert!(
            std::panic::catch_unwind(|| {
                let _owner = circuit();
                panic!("test-only affine owner unwind");
            })
            .is_err()
        );
        assert_eq!(CLEARED.with(std::cell::Cell::get) - before, 9);
    }

    #[test]
    fn affine_output_checks_late_failures_after_valid_prefix() {
        let params = crate::ram_lfe_bfv_parameters_v1();
        let (public, key, _) = crate::derive_identifier_key_material_from_seed(
            &params,
            crate::RAM_LFE_BFV_IDENTIFIER_MAX_INPUT_BYTES,
            b"affine-output-owner",
            b"test",
        )
        .unwrap();
        let encrypt = |values: &[u64], seed: &[u8]| {
            crate::encrypt_from_seed(&params, &public.public_key, values, seed).unwrap()
        };
        let first = encrypt(&[65], b"first-output");
        let second = encrypt(&[66], b"second-output");
        assert_eq!(
            super::super::decrypt_affine_outputs(&public, &key, &[first.clone(), second]).unwrap(),
            b"AB",
        );
        let invalid_tail = encrypt(&[66, 1], b"invalid-tail");
        let error =
            super::super::decrypt_affine_outputs(&public, &key, &[first.clone(), invalid_tail])
                .unwrap_err();
        assert!(error.to_string().contains("non-zero trailing"));
        let invalid_byte = encrypt(&[256], b"invalid-byte");
        let error = super::super::decrypt_affine_outputs(&public, &key, &[first, invalid_byte])
            .unwrap_err();
        assert!(error.to_string().contains("does not fit into u8"));
    }
}
