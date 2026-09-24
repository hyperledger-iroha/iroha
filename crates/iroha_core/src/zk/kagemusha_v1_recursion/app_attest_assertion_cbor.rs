//! Exact original-CBOR binding for the fixed 37-byte Apple assertion profile.
//!
//! This relates private raw bytes to the same assigned authenticator bytes and
//! canonical DER buffer used by SHA/P-256. It grants no monetary authority alone.

use halo2_base::{
    AssignedValue,
    QuantumCell::Constant,
    gates::{GateInstructions as _, RangeInstructions as _, circuit::builder::BaseCircuitBuilder},
};

use crate::zk::{
    kagemusha_p256_curve_gadget::app_attest_der_gadget::P256CanonicalDerV1,
    kagemusha_v1_poseidon::KagemushaPoseidonFieldV1, pasta_sha256::PastaSha256ByteV1,
};

use super::super::canonical_preimage::stream::KagemushaBoundedByteStreamV1;

const RAW_CAP: usize = 142;

fn constants<F: KagemushaPoseidonFieldV1>(bytes: &[u8]) -> Vec<PastaSha256ByteV1<F>> {
    bytes
        .iter()
        .copied()
        .map(PastaSha256ByteV1::constant)
        .collect()
}

/// Copy-bind every byte of the original assertion CBOR to exact `authData` and DER cells.
///
/// A private Boolean chooses either canonical two-key order, but both complete 142-byte
/// zero-padded streams are built with the same fixed gate topology. The active length and
/// every byte are equality-constrained to the original raw witness. The caller must pass
/// `der` built from the very same assigned `r,s` used in the P-256 verifier.
#[allow(clippy::too_many_lines)]
pub(super) fn constrain_original_apple_assertion_37_v1<F: KagemushaPoseidonFieldV1>(
    builder: &mut BaseCircuitBuilder<F>,
    raw_assertion: &[u8],
    authenticator_data: &[AssignedValue<F>; 37],
    der: &P256CanonicalDerV1<F>,
) -> Result<(), String> {
    if raw_assertion.is_empty() || raw_assertion.len() > RAW_CAP {
        return Err("original Apple assertion exceeds fixed CBOR profile".to_owned());
    }
    let range = builder.range_chip();
    let ctx = builder.main(0);
    let gate = range.gate();
    let raw_len = ctx.load_witness(F::from(raw_assertion.len() as u64));
    let raw = (0..RAW_CAP)
        .map(|index| {
            let byte = raw_assertion.get(index).copied().unwrap_or(0);
            let assigned = ctx.load_witness(F::from(u64::from(byte)));
            PastaSha256ByteV1::range_checked(ctx, &range, assigned)
        })
        .collect();
    let original = KagemushaBoundedByteStreamV1::constrain(ctx, &range, raw, raw_len)?;

    gate.assert_is_const(ctx, &authenticator_data[32], &F::from(0x40_u64));
    let mut auth = constants(&[0x71]);
    auth.extend(constants(b"authenticatorData"));
    auth.extend(constants(&[0x58, 0x25]));
    auth.extend(
        authenticator_data
            .iter()
            .copied()
            .map(|value| PastaSha256ByteV1::range_checked(ctx, &range, value)),
    );
    if auth.len() != 57 {
        return Err("fixed Apple authenticator CBOR width changed".to_owned());
    }
    let auth_len = ctx.load_constant(F::from(57_u64));
    let auth = KagemushaBoundedByteStreamV1::constrain(ctx, &range, auth, auth_len)?;

    let der = KagemushaBoundedByteStreamV1::constrain(ctx, &range, der.bytes.to_vec(), der.len)?;
    let der_below_eight = range.is_less_than(ctx, der.actual_len(), Constant(F::from(8_u64)), 7);
    gate.assert_is_const(ctx, &der_below_eight, &F::ZERO);
    let short = range.is_less_than(ctx, der.actual_len(), Constant(F::from(24_u64)), 7);
    let long = gate.not(ctx, short);
    let short_header = gate.add(ctx, der.actual_len(), Constant(F::from(0x40_u64)));
    let header_first = gate.select(ctx, Constant(F::from(0x58_u64)), short_header, long);
    let header_second = gate.mul(ctx, long, der.actual_len());
    let header_len = gate.add(ctx, Constant(F::ONE), long);
    let header_bytes = vec![
        PastaSha256ByteV1::range_checked(ctx, &range, header_first),
        PastaSha256ByteV1::range_checked(ctx, &range, header_second),
    ];
    let header = KagemushaBoundedByteStreamV1::constrain(ctx, &range, header_bytes, header_len)?;
    let mut sig = constants(&[0x69]);
    sig.extend(constants(b"signature"));
    let sig_prefix_len = ctx.load_constant(F::from(10_u64));
    let sig_prefix = KagemushaBoundedByteStreamV1::constrain(ctx, &range, sig, sig_prefix_len)?;
    let sig = sig_prefix.concat(ctx, &range, &header, 12)?;
    let sig = sig.concat(ctx, &range, &der, 84)?;
    let auth_then_sig = auth.concat(ctx, &range, &sig, 141)?;
    let sig_then_auth = sig.concat(ctx, &range, &auth, 141)?;
    let map_len = ctx.load_constant(F::ONE);
    let map = KagemushaBoundedByteStreamV1::constrain(ctx, &range, constants(&[0xa2]), map_len)?;
    let auth_first = map.concat(ctx, &range, &auth_then_sig, RAW_CAP)?;
    let sig_first = map.concat(ctx, &range, &sig_then_auth, RAW_CAP)?;
    ctx.constrain_equal(&original.actual_len(), &auth_first.actual_len());
    ctx.constrain_equal(&auth_first.actual_len(), &sig_first.actual_len());
    let selected_order = raw_assertion.get(1) == Some(&0x71);
    let selected_order = ctx.load_witness(F::from(u64::from(selected_order)));
    gate.assert_bit(ctx, selected_order);
    for ((original, auth_order), sig_order) in original
        .bytes()
        .iter()
        .zip(auth_first.bytes())
        .zip(sig_first.bytes())
    {
        let reconstructed = gate.select(
            ctx,
            auth_order.quantum_cell(),
            sig_order.quantum_cell(),
            selected_order,
        );
        ctx.constrain_equal(
            &reconstructed,
            &original.assigned().expect("original CBOR byte assigned"),
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use halo2_proofs::{
        dev::MockProver,
        halo2curves::pasta::{Fp, Fq},
    };

    fn raw(auth: &[u8; 37], der: &[u8], auth_first: bool) -> Vec<u8> {
        let mut a = vec![0x71];
        a.extend_from_slice(b"authenticatorData");
        a.extend([0x58, 0x25]);
        a.extend_from_slice(auth);
        let mut t = vec![0x69];
        t.extend_from_slice(b"signature");
        if der.len() < 24 {
            t.push(0x40 + der.len() as u8);
        } else {
            t.extend([0x58, der.len() as u8]);
        }
        t.extend_from_slice(der);
        let mut out = vec![0xa2];
        if auth_first {
            out.extend(a);
            out.extend(t);
        } else {
            out.extend(t);
            out.extend(a);
        }
        out
    }

    fn check<F: KagemushaPoseidonFieldV1>(
        raw_bytes: &[u8],
        auth: &[u8; 37],
        der_bytes: &[u8],
    ) -> bool {
        let mut builder = BaseCircuitBuilder::<F>::new(false)
            .use_k(17)
            .use_lookup_bits(16)
            .use_instance_columns(1);
        let range = builder.range_chip();
        let ctx = builder.main(0);
        let auth: [AssignedValue<F>; 37] =
            std::array::from_fn(|index| ctx.load_witness(F::from(u64::from(auth[index]))));
        let bytes = std::array::from_fn(|index| {
            let byte = der_bytes.get(index).copied().unwrap_or(0);
            let assigned = ctx.load_witness(F::from(u64::from(byte)));
            PastaSha256ByteV1::range_checked(ctx, &range, assigned)
        });
        let len = ctx.load_witness(F::from(der_bytes.len() as u64));
        let der = P256CanonicalDerV1 { bytes, len };
        if constrain_original_apple_assertion_37_v1(&mut builder, raw_bytes, &auth, &der).is_err() {
            return false;
        }
        builder.assigned_instances = vec![Vec::new()];
        builder.calculate_params(Some(9));
        MockProver::run(17, &builder, vec![Vec::new()])
            .expect("original Apple CBOR circuit synthesizes")
            .verify()
            .is_ok()
    }

    #[test]
    fn exact_two_key_cbor_accepts_both_orders_and_all_der_widths() {
        let mut auth = [0x37; 37];
        auth[32] = 0x40;
        for der_len in [8, 69, 70, 71, 72] {
            let mut der = vec![0x17; der_len];
            der[0] = 0x30;
            der[1] = (der_len - 2) as u8;
            for order in [true, false] {
                let assertion = raw(&auth, &der, order);
                assert!(check::<Fp>(&assertion, &auth, &der));
                assert!(check::<Fq>(&assertion, &auth, &der));
            }
        }
    }

    #[test]
    fn changed_original_cbor_or_signed_bytes_fail_in_both_pasta_fields() {
        let mut auth = [0x37; 37];
        auth[32] = 0x40;
        let der = [0x30, 6, 0x02, 1, 1, 0x02, 1, 1];
        let assertion = raw(&auth, &der, true);
        let mut changed_auth = auth;
        changed_auth[36] ^= 1;
        let mut changed_der = der;
        changed_der[4] ^= 1;
        let mut unknown_key = assertion.clone();
        unknown_key[2] ^= 1;
        let mut trailing = assertion.clone();
        trailing.push(0);
        let mut nonminimal_bstr = assertion.clone();
        let signature_key = 1 + 57;
        assert_eq!(nonminimal_bstr[signature_key], 0x69);
        nonminimal_bstr[signature_key + 10] = 0x58;
        nonminimal_bstr.insert(signature_key + 11, 8);
        let mut duplicate = vec![0xa2];
        let signature_entry = &assertion[signature_key..];
        duplicate.extend_from_slice(signature_entry);
        duplicate.extend_from_slice(signature_entry);
        let cases = [
            (assertion.as_slice(), &changed_auth, &der[..]),
            (assertion.as_slice(), &auth, &changed_der[..]),
            (unknown_key.as_slice(), &auth, &der[..]),
            (trailing.as_slice(), &auth, &der[..]),
            (nonminimal_bstr.as_slice(), &auth, &der[..]),
            (duplicate.as_slice(), &auth, &der[..]),
        ];
        for (raw, auth, der) in cases {
            assert!(!check::<Fp>(raw, auth, der));
            assert!(!check::<Fq>(raw, auth, der));
        }
    }
}
