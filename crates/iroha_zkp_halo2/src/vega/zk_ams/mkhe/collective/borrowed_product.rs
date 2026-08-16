//! Limb-wise multiplication from an immutable canonical residue source.
use super::{BgvProfile, RnsPolynomial, ZeroizingRns, ZkAmsMkheErrorV1};
pub(in crate::vega::zk_ams::mkhe) struct ZeroizingU64VectorV1(Option<Vec<u64>>);
impl ZeroizingU64VectorV1 {
    fn with_capacity(capacity: usize) -> Result<Self, ZkAmsMkheErrorV1> {
        let mut values = Vec::new();
        values
            .try_reserve_exact(capacity)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        if values.capacity() != capacity {
            return Err(ZkAmsMkheErrorV1::ResourceCeilingExceeded);
        }
        Ok(Self(Some(values)))
    }
    pub(in crate::vega::zk_ams::mkhe) fn values(&self) -> &[u64] {
        self.0.as_deref().unwrap_or_default()
    }
    fn vector_mut(&mut self) -> &mut Vec<u64> {
        self.0.as_mut().expect("zeroizing owner is still armed")
    }
    pub(in crate::vega::zk_ams::mkhe) fn values_mut(&mut self) -> &mut [u64] {
        self.vector_mut()
    }
    fn push(&mut self, value: u64) {
        self.vector_mut().push(value);
    }
    fn extend_from_slice(&mut self, values: &[u64]) -> Result<(), ZkAmsMkheErrorV1> {
        let output = self.vector_mut();
        let required = output
            .len()
            .checked_add(values.len())
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        if required > output.capacity() {
            return Err(ZkAmsMkheErrorV1::ResourceCeilingExceeded);
        }
        output.extend_from_slice(values);
        Ok(())
    }
    fn into_vec(mut self) -> Vec<u64> {
        self.0.take().unwrap_or_default()
    }
}
impl Drop for ZeroizingU64VectorV1 {
    fn drop(&mut self) {
        if let Some(values) = self.0.as_mut() {
            let values = core::hint::black_box(values);
            values.fill(0);
            core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
            let _ = core::hint::black_box(&mut *values);
        }
    }
}
pub(super) fn multiply_public_residues_by_secret_signed_v1(
    left: &[u64],
    right: &[i64],
    profile: &BgvProfile,
) -> Result<ZeroizingRns, ZkAmsMkheErrorV1> {
    let coefficient_count = validate_inputs_v1(left, right, profile)?;
    let mut coefficients = ZeroizingU64VectorV1::with_capacity(coefficient_count)?;
    for limb in 0..profile.moduli.len() {
        let start = limb * profile.ring_degree;
        let end = start + profile.ring_degree;
        let product = negacyclic_multiply_signed_zeroizing_v1(
            &left[start..end],
            right,
            profile.moduli[limb],
            profile.negacyclic_roots[limb],
        )?;
        coefficients.extend_from_slice(product.values())?;
    }
    Ok(ZeroizingRns(RnsPolynomial {
        coefficients: coefficients.into_vec(),
    }))
}
pub(in super::super) fn accumulate_public_residues_times_signed_v1(
    left: &[u64],
    right: &[i64],
    profile: &BgvProfile,
    negate_product: bool,
    output: &mut RnsPolynomial,
) -> Result<(), ZkAmsMkheErrorV1> {
    validate_inputs_v1(left, right, profile)?;
    output.validate(profile)?;
    for limb in 0..profile.moduli.len() {
        let start = limb * profile.ring_degree;
        let end = start + profile.ring_degree;
        let product = negacyclic_multiply_signed_zeroizing_v1(
            &left[start..end],
            right,
            profile.moduli[limb],
            profile.negacyclic_roots[limb],
        )?;
        let modulus = profile.moduli[limb];
        for (output, product) in output.coefficients[start..end]
            .iter_mut()
            .zip(product.values())
        {
            let product = if negate_product && *product != 0 {
                modulus - *product
            } else {
                *product
            };
            *output = super::super::mod_add(*output, product, modulus);
        }
    }
    Ok(())
}
pub(in super::super) fn direct_rkg_one_h0_limb_from_signed_v1(
    common_a: &[u64],
    secret: &[i64],
    ephemeral: &[i64],
    error: &[i64],
    gadget: u64,
    plaintext: u64,
    modulus: u64,
    psi: u64,
) -> Result<Vec<u64>, ZkAmsMkheErrorV1> {
    if common_a.len() != secret.len()
        || secret.len() != ephemeral.len()
        || secret.len() != error.len()
        || common_a.is_empty()
        || common_a.iter().any(|value| *value >= modulus)
        || gadget >= modulus
        || plaintext >= modulus
    {
        return Err(ZkAmsMkheErrorV1::InvalidPolynomial);
    }
    let product = negacyclic_multiply_signed_zeroizing_v1(common_a, ephemeral, modulus, psi)?;
    let mut output = ZeroizingU64VectorV1::with_capacity(common_a.len())?;
    for index in 0..common_a.len() {
        output.push(super::super::mod_add(
            super::super::mod_sub(
                super::super::mod_mul(
                    super::super::signed_mod(secret[index], modulus),
                    gadget,
                    modulus,
                ),
                product.values()[index],
                modulus,
            ),
            super::super::mod_mul(
                super::super::signed_mod(error[index], modulus),
                plaintext,
                modulus,
            ),
            modulus,
        ));
    }
    Ok(output.into_vec())
}
pub(in super::super) fn direct_rkg_one_h1_limb_from_signed_v1(
    common_a: &[u64],
    secret: &[i64],
    error: &[i64],
    plaintext: u64,
    modulus: u64,
    psi: u64,
) -> Result<Vec<u64>, ZkAmsMkheErrorV1> {
    if common_a.len() != secret.len()
        || secret.len() != error.len()
        || common_a.is_empty()
        || common_a.iter().any(|value| *value >= modulus)
        || plaintext >= modulus
    {
        return Err(ZkAmsMkheErrorV1::InvalidPolynomial);
    }
    let product = negacyclic_multiply_signed_zeroizing_v1(common_a, secret, modulus, psi)?;
    let mut output = ZeroizingU64VectorV1::with_capacity(common_a.len())?;
    for index in 0..common_a.len() {
        output.push(super::super::mod_add(
            product.values()[index],
            super::super::mod_mul(
                super::super::signed_mod(error[index], modulus),
                plaintext,
                modulus,
            ),
            modulus,
        ));
    }
    Ok(output.into_vec())
}
fn validate_inputs_v1(
    left: &[u64],
    right: &[i64],
    profile: &BgvProfile,
) -> Result<usize, ZkAmsMkheErrorV1> {
    profile.validate()?;
    super::checked_ring_multiplication_work(profile, 1)?;
    let coefficient_count = profile
        .ring_degree
        .checked_mul(profile.moduli.len())
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    if left.len() != coefficient_count || right.len() != profile.ring_degree {
        return Err(ZkAmsMkheErrorV1::InvalidPolynomial);
    }
    for (limb, residues) in left.chunks_exact(profile.ring_degree).enumerate() {
        if residues
            .iter()
            .any(|residue| *residue >= profile.moduli[limb])
        {
            return Err(ZkAmsMkheErrorV1::InvalidPolynomial);
        }
    }
    Ok(coefficient_count)
}
pub(in crate::vega::zk_ams::mkhe) fn negacyclic_multiply_signed_zeroizing_v1(
    left: &[u64],
    right: &[i64],
    modulus: u64,
    psi: u64,
) -> Result<ZeroizingU64VectorV1, ZkAmsMkheErrorV1> {
    if left.len() != right.len() || left.is_empty() || !left.len().is_power_of_two() {
        return Err(ZkAmsMkheErrorV1::InvalidPolynomial);
    }
    let mut left_twisted = ZeroizingU64VectorV1::with_capacity(left.len())?;
    let mut right_twisted = ZeroizingU64VectorV1::with_capacity(right.len())?;
    let mut twist = 1_u64;
    for (&left, &right) in left.iter().zip(right) {
        left_twisted.push(super::super::mod_mul(left, twist, modulus));
        right_twisted.push(super::super::mod_mul(
            super::super::signed_mod(right, modulus),
            twist,
            modulus,
        ));
        twist = super::super::mod_mul(twist, psi, modulus);
    }
    let root = super::super::mod_mul(psi, psi, modulus);
    super::super::cyclic_ntt(left_twisted.values_mut(), root, modulus);
    super::super::cyclic_ntt(right_twisted.values_mut(), root, modulus);
    for (left, right) in left_twisted
        .values_mut()
        .iter_mut()
        .zip(right_twisted.values().iter().copied())
    {
        *left = super::super::mod_mul(*left, right, modulus);
    }
    super::super::inverse_cyclic_ntt(left_twisted.values_mut(), root, modulus)?;
    let inverse_psi =
        super::super::mod_inverse(psi, modulus).ok_or(ZkAmsMkheErrorV1::InvalidProfile)?;
    let mut untwist = 1_u64;
    for value in left_twisted.values_mut() {
        *value = super::super::mod_mul(*value, untwist, modulus);
        untwist = super::super::mod_mul(untwist, inverse_psi, modulus);
    }
    Ok(left_twisted)
}
#[cfg(test)]
mod tests {
    use super::super::super::PlaintextModulus;
    use super::*;
    const MODULI: [u64; 2] = [2_013_265_921, 1_811_939_329];
    const ROOTS: [u64; 2] = [1_400_279_418, 677_356_115];
    fn profile() -> BgvProfile {
        BgvProfile {
            profile_id: [0x6d; 32],
            ring_degree: 8,
            moduli: &MODULI,
            negacyclic_roots: &ROOTS,
            plaintext_modulus: PlaintextModulus::Tiny(17),
            error_eta: 2,
            hybrid_rns_decomposition: false,
            gadget_base_log: 8,
            gadget_digits: 8,
            max_ciphertext_bytes: 1 << 20,
            max_evaluated_key_bytes: 16 << 20,
            max_round_bytes: 16 << 20,
            max_share_bytes: 4 << 20,
            max_workspace_bytes: 16 << 20,
            max_work_units: 1 << 20,
        }
    }
    #[test]
    fn borrowed_product_matches_owned_rns_multiplication() {
        let profile = profile();
        let left = RnsPolynomial::from_unsigned(&profile, &[1, 3, 5, 7, 9, 11, 13, 15]).unwrap();
        let right_signed = [-1, 0, 1, 1, -1, 0, 1, -1];
        let right = RnsPolynomial::from_signed(&profile, &right_signed).unwrap();
        let expected = left.mul(&right, &profile).unwrap();
        let actual = multiply_public_residues_by_secret_signed_v1(
            &left.coefficients,
            &right_signed,
            &profile,
        )
        .unwrap();
        assert_eq!(&actual.0, &expected);
    }
    #[test]
    fn borrowed_product_source_keeps_secret_tables_zeroizing_and_fallible() {
        let source = include_str!("borrowed_product.rs");
        let production = source.split("#[cfg(test)]").next().unwrap();
        assert!(production.contains("struct ZeroizingU64VectorV1(Option<Vec<u64>>);"));
        assert!(!production.contains("derive(Clone"));
        assert!(!production.contains("impl core::fmt::Debug"));
        assert!(production.contains("impl Drop for ZeroizingU64VectorV1"));
        assert!(production.contains("fn values_mut(&mut self) -> &mut [u64]"));
        assert!(production.contains("try_reserve_exact"));
        assert!(!production.contains(".to_vec()"));
        assert!(!production.contains("vec!["));
        assert!(include_str!("../collective.rs").lines().count() <= 5_000);
    }
}
