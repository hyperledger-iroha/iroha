//! Same-original binding for the ordinary app approval wrapper and financial subject.
//!
//! These are circuit cells, not native authority capabilities. The enclosing native owner must
//! derive every expected field from its held operation, original credential and reserved nonce.
//! Both recursive parities must compose this binding with the actual platform equation, genuine
//! issuer original, predecessor/successor State and complete Guard before ordinary money opens.
// TODO: Compose the original credential encoder/SHA opening and this exact wrapper in both live
// monetary parities, mint and terminal. Keep ordinary runtime refusal until that corridor is complete.

use halo2_base::{
    AssignedValue,
    gates::{GateInstructions as _, RangeInstructions as _, circuit::builder::BaseCircuitBuilder},
};
use iroha_data_model::kagemusha::{
    KAGEMUSHA_APP_OPERATION_APPROVAL_DOMAIN_V1,
    KAGEMUSHA_APP_OPERATION_APPROVAL_MAX_LIFETIME_MS_V1,
    KagemushaAppOperationApprovalSigningLayoutV1 as A,
    KagemushaHardwareSelectionSigningLayoutV1 as S, KagemushaOrdinaryAppCredentialOriginalLayoutV1,
};

use crate::{
    kagemusha_v1_poseidon::KagemushaPoseidonFieldV1,
    pasta_sha256::{PastaSha256ByteV1, PastaSha256JobsV1},
};

use super::{canonical_preimage::assemble_canonical_preimage_v1, guard_bundle::hash};

/// Actual encoder-field cells lent by the complete native credential relation.
/// Platform/security encodings and optional Integrity framing come from the model-owned layout;
/// the Native issuer admission must independently authenticate the exact whole original digest.
#[derive(Clone)]
pub(super) struct OrdinaryCredentialOriginalCellsV1<F: KagemushaPoseidonFieldV1> {
    pub(super) version: [PastaSha256ByteV1<F>; 2],
    pub(super) platform_class: Vec<PastaSha256ByteV1<F>>,
    pub(super) security_level: Vec<PastaSha256ByteV1<F>>,
    pub(super) fixed_digests: [[PastaSha256ByteV1<F>; 32]; 18],
    pub(super) app_public_key: [PastaSha256ByteV1<F>; 65],
    pub(super) scalars: [Vec<PastaSha256ByteV1<F>>; 5],
    pub(super) original_ed_signature: [PastaSha256ByteV1<F>; 64],
    pub(super) play_integrity_fields: Option<[Vec<PastaSha256ByteV1<F>>; 5]>,
    pub(super) issuer_admission: Option<OrdinaryCredentialIssuerCellsV1<F>>,
}

/// Same actual Ed-only SHA and fixed signature authenticated by the governed issuer equation.
#[derive(Clone)]
pub(super) struct OrdinaryCredentialIssuerCellsV1<F: KagemushaPoseidonFieldV1> {
    pub(super) ed_original_sha256: [PastaSha256ByteV1<F>; 32],
    pub(super) signature: [PastaSha256ByteV1<F>; 64],
}

/// Reconstruct the full canonical ordinary credential from assigned semantic fields, including
/// its original Ed signature and derived CRC, then bind SHA to the actual Native-admitted original.
/// This is a same-original relation; it does not replace Native's genuine Ed authority verification.
pub(super) fn constrain_ordinary_credential_original_v1<F: KagemushaPoseidonFieldV1>(
    builder: &mut BaseCircuitBuilder<F>,
    jobs: &mut PastaSha256JobsV1<F>,
    layout: &KagemushaOrdinaryAppCredentialOriginalLayoutV1,
    original: &OrdinaryCredentialOriginalCellsV1<F>,
    expected_digest: &[PastaSha256ByteV1<F>; 32],
) -> Result<[PastaSha256ByteV1<F>; 32], String> {
    let digest = reconstruct_ordinary_credential_original_v1(builder, jobs, layout, original)?;
    let range = builder.range_chip();
    let ctx = builder.main(0);
    for (actual, expected) in digest.iter().zip(expected_digest) {
        let difference = range
            .gate()
            .sub(ctx, actual.quantum_cell(), expected.quantum_cell());
        range.gate().assert_is_const(ctx, &difference, &F::ZERO);
    }
    Ok(digest)
}

/// Canonical union reconstruction; the selected digest is bound by the enclosing public column
/// and genuine issuer equation. Both None and Some layouts must be constructed before selection.
pub(super) fn reconstruct_ordinary_credential_original_v1<F: KagemushaPoseidonFieldV1>(
    builder: &mut BaseCircuitBuilder<F>,
    jobs: &mut PastaSha256JobsV1<F>,
    layout: &KagemushaOrdinaryAppCredentialOriginalLayoutV1,
    original: &OrdinaryCredentialOriginalCellsV1<F>,
) -> Result<[PastaSha256ByteV1<F>; 32], String> {
    if layout.original.end != layout.bytes.len()
        || layout.original.start >= layout.original.end
        || layout.play_integrity_bytes.is_some() != original.play_integrity_fields.is_some()
    {
        return Err("ordinary credential original/Integrity layout differs".to_owned());
    }
    let prefix = layout.bytes[..layout.original.start]
        .iter()
        .copied()
        .collect::<Option<Vec<_>>>()
        .ok_or_else(|| "ordinary credential digest framing is not fixed".to_owned())?;
    // Borrow both original fields and local issuer framing through the same collection scope.
    fn add_raw<'a, F: KagemushaPoseidonFieldV1>(
        original_range: &core::ops::Range<usize>,
        ranges: &mut Vec<core::ops::Range<usize>>,
        fields: &mut Vec<&'a [PastaSha256ByteV1<F>]>,
        positions: &[usize],
        values: &'a [PastaSha256ByteV1<F>],
    ) -> Result<(), String> {
        if positions.len() != values.len() {
            return Err("ordinary credential raw field width differs".to_owned());
        }
        for (position, value) in positions.iter().zip(values) {
            let slot = *position..*position + 1;
            if slot.start < original_range.start || slot.end > original_range.end {
                return Err("ordinary credential semantic range exceeds original".to_owned());
            }
            ranges.push(slot.start - original_range.start..slot.end - original_range.start);
            fields.push(core::slice::from_ref(value));
        }
        Ok(())
    }
    let original_range = &layout.original;
    let version = [
        PastaSha256ByteV1::constant(1),
        PastaSha256ByteV1::constant(0),
    ];
    let purpose = [PastaSha256ByteV1::constant(1)];
    let mut ranges = Vec::new();
    let mut fields = Vec::new();
    add_raw(
        original_range,
        &mut ranges,
        &mut fields,
        &layout.version_bytes,
        &original.version,
    )?;
    add_raw(
        original_range,
        &mut ranges,
        &mut fields,
        &layout.platform_class_bytes,
        &original.platform_class,
    )?;
    add_raw(
        original_range,
        &mut ranges,
        &mut fields,
        &layout.security_level_bytes,
        &original.security_level,
    )?;
    for (positions, values) in layout
        .fixed_digest_bytes
        .iter()
        .zip(&original.fixed_digests)
    {
        add_raw(original_range, &mut ranges, &mut fields, positions, values)?;
    }
    add_raw(
        original_range,
        &mut ranges,
        &mut fields,
        &layout.app_public_key_bytes,
        &original.app_public_key,
    )?;
    for (positions, values) in layout.scalar_bytes.iter().zip(&original.scalars) {
        add_raw(original_range, &mut ranges, &mut fields, positions, values)?;
    }
    add_raw(
        original_range,
        &mut ranges,
        &mut fields,
        &layout.signature_bytes,
        &original.original_ed_signature,
    )?;
    if let (Some(pi_positions), Some(pi_fields)) = (
        &layout.play_integrity_bytes,
        &original.play_integrity_fields,
    ) {
        for (positions, values) in pi_positions.iter().zip(pi_fields) {
            add_raw(original_range, &mut ranges, &mut fields, positions, values)?;
        }
    }
    if let Some(layout) = &layout.issuer_admission_layout {
        let admission = original
            .issuer_admission
            .as_ref()
            .ok_or("ordinary original omits mandatory issuer admission cells")?;
        add_raw(
            original_range,
            &mut ranges,
            &mut fields,
            &layout.version_bytes,
            &version,
        )?;
        add_raw(
            original_range,
            &mut ranges,
            &mut fields,
            core::slice::from_ref(&layout.purpose_byte),
            &purpose,
        )?;
        for (positions, values) in layout.fixed_digest_bytes.iter().zip([
            &original.fixed_digests[6],
            &original.fixed_digests[7],
            &admission.ed_original_sha256,
        ]) {
            add_raw(original_range, &mut ranges, &mut fields, positions, values)?;
        }
        add_raw(
            original_range,
            &mut ranges,
            &mut fields,
            &layout.signature_bytes,
            &admission.signature,
        )?;
    }
    let range = builder.range_chip();
    let ctx = builder.main(0);
    let frame = assemble_canonical_preimage_v1(
        ctx,
        &range,
        &layout.bytes[layout.original.clone()],
        &ranges,
        &fields,
    )?;
    let mut preimage = prefix
        .into_iter()
        .map(PastaSha256ByteV1::constant)
        .collect::<Vec<_>>();
    preimage.extend(frame);
    hash(ctx, jobs, preimage)
}

/// Assigned originals lent by the actual financial operation/current credential relation.
/// Nonce and time must originate in the native durable attempt, never in a response or assertion.
pub(super) struct OrdinaryApprovalOriginalCellsV1<F: KagemushaPoseidonFieldV1> {
    pub(super) operation_id: [PastaSha256ByteV1<F>; 32],
    pub(super) nonce: [PastaSha256ByteV1<F>; 32],
    pub(super) account_binding: [PastaSha256ByteV1<F>; 32],
    pub(super) authority_policy_digest: [PastaSha256ByteV1<F>; 32],
    pub(super) attested_key_id: [PastaSha256ByteV1<F>; 32],
    pub(super) enrollment_digest: [PastaSha256ByteV1<F>; 32],
    pub(super) normalized_guard_digest: [PastaSha256ByteV1<F>; 32],
    pub(super) issued_at_ms: AssignedValue<F>,
    pub(super) expires_at_ms: AssignedValue<F>,
}

/// Copy-bind every wrapper field and SHA(full S) to the same original financial subject.
/// `canonical_s` must already be derived from actual assigned State, Guard, candidate and terminal
/// cells. The returned array is exactly the bytes given to Android ECDSA or Apple's clientDataHash.
/// Queue realization and the original platform signature equation remain mandatory.
pub(super) fn constrain_ordinary_approval_wrapper_v1<F: KagemushaPoseidonFieldV1>(
    builder: &mut BaseCircuitBuilder<F>,
    jobs: &mut PastaSha256JobsV1<F>,
    wrapper: &[AssignedValue<F>; A::TOTAL_BYTES],
    canonical_s: &[AssignedValue<F>; S::TOTAL_BYTES],
    original: &OrdinaryApprovalOriginalCellsV1<F>,
) -> Result<[PastaSha256ByteV1<F>; A::TOTAL_BYTES], String> {
    let range = builder.range_chip();
    let ctx = builder.main(0);
    let gate = range.gate();
    let framed = wrapper
        .iter()
        .copied()
        .map(|cell| PastaSha256ByteV1::range_checked(ctx, &range, cell))
        .collect::<Vec<_>>();
    let mut prefix = KAGEMUSHA_APP_OPERATION_APPROVAL_DOMAIN_V1.to_vec();
    prefix.extend_from_slice(&(A::BODY.len() as u64).to_le_bytes());
    prefix.extend_from_slice(&1_u16.to_le_bytes());
    for (cell, byte) in wrapper.iter().zip(prefix) {
        gate.assert_is_const(ctx, cell, &F::from(u64::from(byte)));
    }
    // Bootstrap/terminal MonetaryTransition and pre-candidate PrepareTransition are distinct
    // signed purposes. The actual State/terminal consumer must select its exact allowed one.
    let purpose = wrapper[A::PURPOSE.start];
    let first = gate.sub(ctx, purpose, halo2_base::QuantumCell::Constant(F::ONE));
    let second = gate.sub(ctx, purpose, halo2_base::QuantumCell::Constant(F::from(2)));
    let invalid = gate.mul(ctx, first, second);
    gate.assert_is_const(ctx, &invalid, &F::ZERO);
    let mut subject_prefix = S::DOMAIN_BYTES.to_vec();
    subject_prefix.extend_from_slice(&(S::BODY_BYTES as u64).to_le_bytes());
    subject_prefix.extend_from_slice(&1_u16.to_le_bytes());
    for (cell, byte) in canonical_s.iter().zip(subject_prefix) {
        gate.assert_is_const(ctx, cell, &F::from(u64::from(byte)));
    }
    let subject = canonical_s
        .iter()
        .copied()
        .map(|cell| PastaSha256ByteV1::range_checked(ctx, &range, cell))
        .collect::<Vec<_>>();
    let subject_digest = hash(ctx, jobs, subject)?;
    let bindings = [
        (A::OPERATION_ID, &original.operation_id),
        (A::NONCE, &original.nonce),
        (A::ACCOUNT_BINDING, &original.account_binding),
        (
            A::AUTHORITY_POLICY_DIGEST,
            &original.authority_policy_digest,
        ),
        (A::ATTESTED_KEY_ID, &original.attested_key_id),
        (A::ENROLLMENT_DIGEST, &original.enrollment_digest),
        (A::SUBJECT_SIGNING_DIGEST, &subject_digest),
        (
            A::NORMALIZED_GUARD_DIGEST,
            &original.normalized_guard_digest,
        ),
    ];
    for (slot, original_bytes) in bindings {
        // Mandatory wrapper identities are bounded bytes, so their sum cannot wrap. Rejecting
        // zero is an actual relation constraint, independent of host canonical parsing.
        let sum = gate.sum(ctx, wrapper[slot.clone()].iter().copied());
        let zero = gate.is_zero(ctx, sum);
        gate.assert_is_const(ctx, &zero, &F::ZERO);
        for (actual, expected) in wrapper[slot].iter().zip(original_bytes) {
            let difference = gate.sub(ctx, *actual, expected.quantum_cell());
            gate.assert_is_const(ctx, &difference, &F::ZERO);
        }
    }
    // The wrapper's valid interval is native lease metadata, independent of the assertion counter.
    range.range_check(ctx, original.issued_at_ms, 64);
    range.range_check(ctx, original.expires_at_ms, 64);
    let zero_issued = gate.is_zero(ctx, original.issued_at_ms);
    gate.assert_is_const(ctx, &zero_issued, &F::ZERO);
    let valid_interval = range.is_less_than(ctx, original.issued_at_ms, original.expires_at_ms, 64);
    gate.assert_is_const(ctx, &valid_interval, &F::ONE);
    let lifetime = gate.sub(ctx, original.expires_at_ms, original.issued_at_ms);
    range.range_check(ctx, lifetime, 17);
    let bounded_lifetime = range.is_less_than(
        ctx,
        lifetime,
        halo2_base::QuantumCell::Constant(F::from(
            KAGEMUSHA_APP_OPERATION_APPROVAL_MAX_LIFETIME_MS_V1 + 1,
        )),
        17,
    );
    gate.assert_is_const(ctx, &bounded_lifetime, &F::ONE);
    for (slot, value) in [
        (A::ISSUED_AT_MS, original.issued_at_ms),
        (A::EXPIRES_AT_MS, original.expires_at_ms),
    ] {
        let bits = gate.num_to_bits(ctx, value, 64);
        for (actual, byte_bits) in wrapper[slot].iter().zip(bits.chunks(8)) {
            let byte = gate.inner_product(
                ctx,
                byte_bits.iter().copied(),
                (0..8).map(|bit| halo2_base::QuantumCell::Constant(F::from(1_u64 << bit))),
            );
            ctx.constrain_equal(actual, &byte);
        }
    }
    framed
        .try_into()
        .map_err(|_| "ordinary approval wrapper width drift".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pasta_sha256::PastaSha256ConfigV1;
    use halo2_base::gates::circuit::{BaseCircuitParams, BaseConfig};
    use halo2_proofs::{
        circuit::{Layouter, V1},
        dev::MockProver,
        halo2curves::pasta::{Fp, Fq},
        plonk::{Circuit, ConstraintSystem, Error},
    };
    use iroha_data_model::{
        NetworkId,
        kagemusha::{
            KagemushaAppOperationApprovalChallengeV1, KagemushaAppOperationApprovalPurposeV1,
            KagemushaHardwareTransitionSelectionV1, KagemushaOperationKindV1,
        },
    };
    use sha2::{Digest as _, Sha256};

    const K: u32 = 17;
    const UNUSABLE: usize = 9;

    #[derive(Clone, Debug)]
    struct Config<F: halo2_base::utils::ScalarField> {
        base: BaseConfig<F>,
        sha: PastaSha256ConfigV1,
    }
    #[derive(Clone)]
    struct BindingCircuit<F: KagemushaPoseidonFieldV1> {
        builder: BaseCircuitBuilder<F>,
        jobs: PastaSha256JobsV1<F>,
    }
    impl<F: KagemushaPoseidonFieldV1> Circuit<F> for BindingCircuit<F> {
        type Config = Config<F>;
        type FloorPlanner = V1;
        type Params = BaseCircuitParams;

        fn params(&self) -> Self::Params {
            self.builder.config_params.clone()
        }
        fn without_witnesses(&self) -> Self {
            Self {
                builder: self.builder.deep_clone().unknown(true),
                jobs: self.jobs.unknown(),
            }
        }
        fn configure_with_params(
            meta: &mut ConstraintSystem<F>,
            params: Self::Params,
        ) -> Self::Config {
            let usable_rows = (1_usize << params.k) - UNUSABLE;
            let mut base = BaseConfig::configure(meta, params);
            base.set_usable_rows(usable_rows);
            Config {
                base,
                sha: PastaSha256ConfigV1::configure(meta),
            }
        }
        fn configure(_: &mut ConstraintSystem<F>) -> Self::Config {
            unreachable!("ordinary binding test uses fixed circuit parameters")
        }
        fn synthesize(
            &self,
            config: Self::Config,
            mut layouter: impl Layouter<F>,
        ) -> Result<(), Error> {
            <BaseCircuitBuilder<F> as Circuit<F>>::synthesize(
                &self.builder,
                config.base,
                layouter.namespace(|| "ordinary original binding Base"),
            )?;
            self.jobs.synthesize(
                &config.sha,
                &mut layouter,
                &self.builder.core().copy_manager,
                (1_usize << K) - UNUSABLE,
            )
        }
    }

    fn original() -> KagemushaAppOperationApprovalChallengeV1 {
        let index = u128::from(u32::MAX) + 50;
        let subject = KagemushaHardwareTransitionSelectionV1 {
            version: 1,
            release_id: [1; 32],
            provider_policy_root: [2; 32],
            app_policy_digest: [3; 32],
            credential_id: [4; 32],
            network_id: NetworkId::from_genesis_hash(iroha_crypto::HashOf::from_untyped_unchecked(
                iroha_crypto::Hash::from_marked_bytes([5; 32])
                    .expect("marked network identity fixture"),
            )),
            lane_commitment: [6; 32],
            hardware_profile_id: [7; 32],
            policy_epoch: 1,
            hardware_epoch_id: [8; 32],
            hardware_epoch_generation: 1,
            operation_kind: KagemushaOperationKindV1::MintFold,
            transition_statement_digest: [9; 32],
            candidate_envelope_digest: [0; 32],
            terminal_body_commitment: [0; 32],
            secure_index_before: index,
            secure_index_after: index + 1,
        };
        KagemushaAppOperationApprovalChallengeV1 {
            version: 1,
            purpose: KagemushaAppOperationApprovalPurposeV1::MonetaryTransition,
            operation_id: [10; 32],
            nonce: [11; 32],
            account_binding: [12; 32],
            authority_policy_digest: [13; 32],
            attested_key_id: [14; 32],
            enrollment_digest: [15; 32],
            subject_signing_digest: Sha256::digest(subject.canonical_signing_bytes().unwrap())
                .into(),
            normalized_guard_digest: [16; 32],
            issued_at_ms: 1000,
            expires_at_ms: 121_000,
            subject,
        }
    }

    fn check<F: KagemushaPoseidonFieldV1>(
        wrapper_mutation: Option<usize>,
        subject_mutation: Option<usize>,
        native_expiry: u64,
    ) -> bool {
        check_selected::<F>(
            wrapper_mutation,
            subject_mutation,
            native_expiry,
            None,
            None,
        )
    }

    fn check_selected<F: KagemushaPoseidonFieldV1>(
        wrapper_mutation: Option<usize>,
        subject_mutation: Option<usize>,
        native_expiry: u64,
        zero_native_scope: Option<usize>,
        purpose: Option<u8>,
    ) -> bool {
        // Public synthetic model bytes only. These tests authenticate no certificate or wallet.
        let mut expected = original();
        let mut wrapper = expected.canonical_signing_bytes().unwrap();
        let mut subject = expected.subject.canonical_signing_bytes().unwrap();
        if let Some(selector) = zero_native_scope {
            // Match both copies to zero, so refusal must come from the relation's nonzero
            // admission rather than a mismatching independently selected original.
            let (selected, field) = match selector {
                0 => (&mut expected.operation_id, A::OPERATION_ID),
                1 => (&mut expected.nonce, A::NONCE),
                2 => (&mut expected.account_binding, A::ACCOUNT_BINDING),
                3 => (
                    &mut expected.authority_policy_digest,
                    A::AUTHORITY_POLICY_DIGEST,
                ),
                4 => (&mut expected.attested_key_id, A::ATTESTED_KEY_ID),
                5 => (&mut expected.enrollment_digest, A::ENROLLMENT_DIGEST),
                6 => (
                    &mut expected.normalized_guard_digest,
                    A::NORMALIZED_GUARD_DIGEST,
                ),
                _ => panic!("unknown native scope test selector"),
            };
            *selected = [0; 32];
            wrapper[field].fill(0);
        }
        if let Some(purpose) = purpose {
            wrapper[A::PURPOSE.start] = purpose;
        }
        if let Some(offset) = wrapper_mutation {
            wrapper[offset] ^= 1;
        }
        if let Some(offset) = subject_mutation {
            subject[offset] ^= 1;
            // Recomputing the response-selected hash cannot change the independently selected S.
            wrapper[A::SUBJECT_SIGNING_DIGEST].copy_from_slice(&Sha256::digest(&subject));
        }
        wrapper[A::EXPIRES_AT_MS].copy_from_slice(&native_expiry.to_le_bytes());
        let mut builder = BaseCircuitBuilder::<F>::new(false)
            .use_k(K as usize)
            .use_lookup_bits(16);
        let range = builder.range_chip();
        let ctx = builder.main(0);
        let wrapper = std::array::from_fn(|i| ctx.load_witness(F::from(u64::from(wrapper[i]))));
        let actual_subject =
            std::array::from_fn(|i| ctx.load_witness(F::from(u64::from(subject[i]))));
        // S is independently derived from the retained financial State/Guard in production.
        // Pin it here to the selected original so recomputed digest substitution exercises that
        // same essential copy boundary, without pretending this synthetic fixture is a State.
        let selected_subject = expected.subject.canonical_signing_bytes().unwrap();
        for (actual, selected) in actual_subject.iter().zip(selected_subject) {
            range
                .gate()
                .assert_is_const(ctx, actual, &F::from(u64::from(selected)));
        }
        let native = OrdinaryApprovalOriginalCellsV1 {
            operation_id: expected.operation_id.map(PastaSha256ByteV1::constant),
            nonce: expected.nonce.map(PastaSha256ByteV1::constant),
            account_binding: expected.account_binding.map(PastaSha256ByteV1::constant),
            authority_policy_digest: expected
                .authority_policy_digest
                .map(PastaSha256ByteV1::constant),
            attested_key_id: expected.attested_key_id.map(PastaSha256ByteV1::constant),
            enrollment_digest: expected.enrollment_digest.map(PastaSha256ByteV1::constant),
            normalized_guard_digest: expected
                .normalized_guard_digest
                .map(PastaSha256ByteV1::constant),
            issued_at_ms: ctx.load_witness(F::from(expected.issued_at_ms)),
            expires_at_ms: ctx.load_witness(F::from(native_expiry)),
        };
        let mut jobs = PastaSha256JobsV1::default();
        constrain_ordinary_approval_wrapper_v1(
            &mut builder,
            &mut jobs,
            &wrapper,
            &actual_subject,
            &native,
        )
        .unwrap();
        builder.calculate_params(Some(UNUSABLE));
        let circuit = BindingCircuit { builder, jobs };
        MockProver::run(K, &circuit, vec![])
            .expect("ordinary wrapper original binding synthesizes")
            .verify()
            .is_ok()
    }

    #[test]
    fn exact_native_subject_wrapper_and_original_interval_bind_in_both_pasta_fields() {
        assert!(check::<Fp>(None, None, 121_000));
        assert!(check::<Fq>(None, None, 121_000));
    }
    #[test]
    fn response_selected_operation_nonce_credential_or_guard_cannot_replace_native_original() {
        for offset in [
            A::OPERATION_ID.start,
            A::NONCE.start,
            A::ACCOUNT_BINDING.start,
            A::AUTHORITY_POLICY_DIGEST.start,
            A::ATTESTED_KEY_ID.start,
            A::ENROLLMENT_DIGEST.start,
            A::NORMALIZED_GUARD_DIGEST.start,
        ] {
            assert!(!check::<Fp>(Some(offset), None, 121_000));
            assert!(!check::<Fq>(Some(offset), None, 121_000));
        }
    }
    #[test]
    fn recomputed_foreign_subject_and_renewed_interval_are_rejected_in_both_pasta_fields() {
        for offset in [S::DOMAIN.start, S::TRANSITION_STATEMENT_DIGEST.start] {
            assert!(!check::<Fp>(None, Some(offset), 121_000));
            assert!(!check::<Fq>(None, Some(offset), 121_000));
        }
        for expiry in [1000, 999, 121_001] {
            assert!(!check::<Fp>(None, None, expiry));
            assert!(!check::<Fq>(None, None, expiry));
        }
    }

    #[test]
    fn matching_zero_native_scopes_are_rejected_without_host_shape_admission() {
        for selector in 0..7 {
            assert!(!check_selected::<Fp>(
                None,
                None,
                121_000,
                Some(selector),
                None
            ));
            assert!(!check_selected::<Fq>(
                None,
                None,
                121_000,
                Some(selector),
                None
            ));
        }
    }

    #[test]
    fn only_the_two_native_purposes_share_the_wrapper_relation() {
        // The enclosing State/terminal consumer additionally selects its one exact purpose.
        assert!(check_selected::<Fp>(None, None, 121_000, None, Some(2)));
        assert!(check_selected::<Fq>(None, None, 121_000, None, Some(2)));
        for purpose in [0, 3, u8::MAX] {
            assert!(!check_selected::<Fp>(
                None,
                None,
                121_000,
                None,
                Some(purpose)
            ));
            assert!(!check_selected::<Fq>(
                None,
                None,
                121_000,
                None,
                Some(purpose)
            ));
        }
    }

    fn check_credential_original<F: KagemushaPoseidonFieldV1>(
        apple: bool,
        mutated_field: Option<usize>,
        mutate_signature: bool,
    ) -> bool {
        use iroha_data_model::testing::ordinary_app_enrollment::KagemushaOrdinaryRetailEnrollmentFixtureV1;
        let fixture = KagemushaOrdinaryRetailEnrollmentFixtureV1::new(apple);
        let admitted = fixture.verify(300).unwrap();
        let credential = admitted.app_credential();
        let original =
            iroha_data_model::kagemusha::KagemushaOrdinaryAppCredentialV1::decode_canonical_exact(
                credential.original(),
            )
            .unwrap();
        let layout = original.original_preimage_layout().unwrap();
        let mut preimage = layout.bytes[..layout.original.start]
            .iter()
            .map(|b| b.expect("model-owned fixed original digest domain/length"))
            .collect::<Vec<_>>();
        preimage.extend(original.canonical_bytes().unwrap());
        assert_eq!(
            <[u8; 32]>::from(Sha256::digest(&preimage)),
            credential.digest()
        );
        if let Some(field) = mutated_field {
            let position = match field {
                18 => layout.fixed_digest_bytes[15][0],
                21 => layout.app_public_key_bytes[1],
                23 => layout.scalar_bytes[1][0],
                26 => layout.scalar_bytes[4][0],
                _ => panic!("test mutation must name an actual model-owned raw field"),
            };
            preimage[position] ^= 1;
        }
        if mutate_signature {
            preimage[layout.signature_bytes[0]] ^= 1;
        }
        let mut builder = BaseCircuitBuilder::<F>::new(false)
            .use_k(K as usize)
            .use_lookup_bits(16);
        let range = builder.range_chip();
        let ctx = builder.main(0);
        let mut assign = |bytes: &[u8]| {
            bytes
                .iter()
                .map(|b| {
                    let cell = ctx.load_witness(F::from(u64::from(*b)));
                    PastaSha256ByteV1::range_checked(ctx, &range, cell)
                })
                .collect::<Vec<_>>()
        };
        let mut assign_positions = |positions: &[usize]| {
            positions
                .iter()
                .map(|position| assign(&preimage[*position..*position + 1])[0])
                .collect::<Vec<_>>()
        };
        let fields = OrdinaryCredentialOriginalCellsV1 {
            version: assign_positions(&layout.version_bytes).try_into().unwrap(),
            platform_class: assign_positions(&layout.platform_class_bytes),
            security_level: assign_positions(&layout.security_level_bytes),
            fixed_digests: core::array::from_fn(|i| {
                assign_positions(&layout.fixed_digest_bytes[i])
                    .try_into()
                    .unwrap()
            }),
            app_public_key: assign_positions(&layout.app_public_key_bytes)
                .try_into()
                .unwrap(),
            scalars: core::array::from_fn(|i| assign_positions(&layout.scalar_bytes[i])),
            original_ed_signature: assign_positions(&layout.signature_bytes)
                .try_into()
                .unwrap(),
            play_integrity_fields: layout
                .play_integrity_bytes
                .as_ref()
                .map(|positions| core::array::from_fn(|i| assign_positions(&positions[i]))),
            issuer_admission: layout.issuer_admission_layout.as_ref().map(|issuer| {
                OrdinaryCredentialIssuerCellsV1 {
                    ed_original_sha256: assign_positions(&issuer.fixed_digest_bytes[2])
                        .try_into()
                        .unwrap(),
                    signature: assign_positions(&issuer.signature_bytes)
                        .try_into()
                        .unwrap(),
                }
            }),
        };
        let expected = credential.digest().map(PastaSha256ByteV1::constant);
        let mut jobs = PastaSha256JobsV1::default();
        constrain_ordinary_credential_original_v1(
            &mut builder,
            &mut jobs,
            &layout,
            &fields,
            &expected,
        )
        .unwrap();
        builder.calculate_params(Some(UNUSABLE));
        let circuit = BindingCircuit { builder, jobs };
        MockProver::run(K, &circuit, vec![])
            .expect("ordinary full original SHA/CRC relation synthesizes")
            .verify()
            .is_ok()
    }

    #[test]
    fn genuine_ed_original_digest_and_canonical_crc_bind_in_both_pasta_fields() {
        for apple in [false, true] {
            assert!(check_credential_original::<Fp>(apple, None, false));
            assert!(check_credential_original::<Fq>(apple, None, false));
        }
    }

    #[test]
    fn financial_commitment_key_epoch_or_original_ed_signature_cannot_substitute_native_original() {
        // Exact model-declared encoded field indices: financial commitment, original SEC1,
        // native financial epoch and independent App Attest floor. No hand-written byte offsets.
        for field in [18, 21, 23, 26] {
            assert!(!check_credential_original::<Fp>(true, Some(field), false));
            assert!(!check_credential_original::<Fq>(true, Some(field), false));
        }
        assert!(!check_credential_original::<Fp>(true, None, true));
        assert!(!check_credential_original::<Fq>(true, None, true));
    }
}
