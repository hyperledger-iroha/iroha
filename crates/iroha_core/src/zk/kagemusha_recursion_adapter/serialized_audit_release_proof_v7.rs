// Non-shipping release-proof driver for the V7 serialized-advice graph.
//
// This source deliberately remains behind `kagemusha-generation-memory-lab`
// and behind the hard-false V7 release gate.  It creates no artifact files.

const KAGEMUSHA_SERIALIZED_RELEASE_CARRIER_VERSION_V7: u16 = 7;
const KAGEMUSHA_SERIALIZED_NULL_CARRIER_ROLE_V7: u8 = 0;
const KAGEMUSHA_SERIALIZED_PARAMS_BYTES_V7: usize = 8_388_676;
const KAGEMUSHA_SERIALIZED_VERIFYING_KEY_BYTES_V7: usize = 20_394;
const KAGEMUSHA_SERIALIZED_PROVING_KEY_BYTES_V7: u64 = 5_356_151_726;
const KAGEMUSHA_SERIALIZED_STEP_PROOF_BYTES_V7: usize = 93_184;
const KAGEMUSHA_SERIALIZED_RAW_PROOF_PAIR_BYTES_V7: usize = 186_368;
const KAGEMUSHA_SERIALIZED_STEP_PROOF_MAX_BYTES_V7: usize = 192 * 1024;
const KAGEMUSHA_SERIALIZED_REVIEWED_PEAK_BYTES_V7: u64 = 53_126_388_928;
const KAGEMUSHA_SERIALIZED_REQUIRED_TERMINAL_IPA_DECISIONS_V7: usize = 26;

fn kagemusha_serialized_release_manifest_template_v7()
-> super::kagemusha_serialized_audit_v7::KagemushaSerializedAuditManifestV7 {
    let phases = vec![0; 412];
    super::kagemusha_serialized_audit_v7::KagemushaSerializedAuditManifestV7 {
        k: KAGEMUSHA_STEP_CIRCUIT_MINIMUM_K_V4,
        // Preparation replaces every identity below with bytes derived from
        // the guarded Params/VKs.  Distinct nonzero placeholders make any
        // accidental pre-preparation use fail closed.
        step_eq_params_sha256: [0x11; 32],
        step_ep_params_sha256: [0x12; 32],
        step_eq_vk_sha256: [0x22; 32],
        step_ep_vk_sha256: [0x33; 32],
        step_eq_advice_phases: phases.clone(),
        step_ep_advice_phases: phases,
        step_eq_serialized_column: 411,
        step_ep_serialized_column: 411,
        step_eq_phase_zero_rank: 411,
        step_ep_phase_zero_rank: 411,
        step_eq_constraint_degree: 9,
        step_ep_constraint_degree: 9,
        step_eq_fixed_columns: 339,
        step_ep_fixed_columns: 339,
        step_eq_permutation_columns: 298,
        step_ep_permutation_columns: 298,
        step_eq_blinding_factors: 8,
        step_ep_blinding_factors: 8,
        minimum_unusable_rows:
            super::kagemusha_serialized_audit_v7::SERIALIZED_EXPECTED_UNUSABLE_ROWS_V7,
        step_eq_proof_bytes:
            super::kagemusha_serialized_audit_v7::SERIALIZED_EXPECTED_STEP_PROOF_BYTES_V7,
        step_ep_proof_bytes:
            super::kagemusha_serialized_audit_v7::SERIALIZED_EXPECTED_STEP_PROOF_BYTES_V7,
        eq_coefficient_count: 10_111,
        ep_coefficient_count: 10_111,
        public_instance_cells: 70,
        current_join_offset: 57,
        parent_digest_offset: 67,
        live_selector_offset: 69,
    }
}

fn prepare_kagemusha_serialized_eq_proving_key_v7(
    proving_key: halo2_proofs::plonk::ProvingKey<halo2_proofs::halo2curves::pasta::EqAffine>,
    expected_verifying_key: &halo2_proofs::plonk::VerifyingKey<
        halo2_proofs::halo2curves::pasta::EqAffine,
    >,
    expected_sha256: Option<[u8; 32]>,
) -> Result<
    KagemushaSerializedPreparedProvingKeyV7<halo2_proofs::halo2curves::pasta::EqAffine>,
    String,
> {
    use halo2_proofs::SerdeFormat;

    if proving_key.get_vk().to_bytes(SerdeFormat::Processed)
        != expected_verifying_key.to_bytes(SerdeFormat::Processed)
    {
        return Err("Kagemusha V7 Eq proving key embeds a different final VK".to_owned());
    }
    let mut raw = KagemushaInfallibleArtifactSpoolWriterV4::new("V7 Eq proving key")?;
    let size_bytes = {
        let mut bounded = KagemushaBoundedProvingKeyWriterV5::new(&mut raw);
        proving_key
            .write_streaming(&mut bounded, SerdeFormat::Processed)
            .map_err(|error| {
                format!("failed to stream Kagemusha V7 Eq processed proving key: {error}")
            })?;
        bounded.finish("V7 Eq proving key")?
    };
    let mut spool = raw.finish("V7 Eq proving key")?;
    if size_bytes != KAGEMUSHA_SERIALIZED_PROVING_KEY_BYTES_V7
        || spool.size_bytes() != size_bytes
        || spool.sha256() == [0; 32]
        || expected_sha256.is_some_and(|expected| expected != spool.sha256())
    {
        return Err("Kagemusha V7 Eq proving-key spool identity/size drifted".to_owned());
    }
    // A complete bounded reread authenticates the temporary custody copy
    // before the in-memory key is allowed to reach the Prover graph.
    spool.copy_to(&mut std::io::sink())?;
    Ok(KagemushaSerializedPreparedProvingKeyV7 {
        key: proving_key,
        size_bytes,
        sha256: spool.sha256(),
    })
}

fn prepare_kagemusha_serialized_ep_proving_key_v7(
    proving_key: halo2_proofs::plonk::ProvingKey<halo2_proofs::halo2curves::pasta::EpAffine>,
    expected_verifying_key: &halo2_proofs::plonk::VerifyingKey<
        halo2_proofs::halo2curves::pasta::EpAffine,
    >,
    expected_sha256: Option<[u8; 32]>,
) -> Result<
    KagemushaSerializedPreparedProvingKeyV7<halo2_proofs::halo2curves::pasta::EpAffine>,
    String,
> {
    use halo2_proofs::SerdeFormat;

    if proving_key.get_vk().to_bytes(SerdeFormat::Processed)
        != expected_verifying_key.to_bytes(SerdeFormat::Processed)
    {
        return Err("Kagemusha V7 Ep proving key embeds a different final VK".to_owned());
    }
    let mut raw = KagemushaInfallibleArtifactSpoolWriterV4::new("V7 Ep proving key")?;
    let size_bytes = {
        let mut bounded = KagemushaBoundedProvingKeyWriterV5::new(&mut raw);
        proving_key
            .write_streaming(&mut bounded, SerdeFormat::Processed)
            .map_err(|error| {
                format!("failed to stream Kagemusha V7 Ep processed proving key: {error}")
            })?;
        bounded.finish("V7 Ep proving key")?
    };
    let mut spool = raw.finish("V7 Ep proving key")?;
    if size_bytes != KAGEMUSHA_SERIALIZED_PROVING_KEY_BYTES_V7
        || spool.size_bytes() != size_bytes
        || spool.sha256() == [0; 32]
        || expected_sha256.is_some_and(|expected| expected != spool.sha256())
    {
        return Err("Kagemusha V7 Ep proving-key spool identity/size drifted".to_owned());
    }
    spool.copy_to(&mut std::io::sink())?;
    Ok(KagemushaSerializedPreparedProvingKeyV7 {
        key: proving_key,
        size_bytes,
        sha256: spool.sha256(),
    })
}

#[derive(Clone)]
struct KagemushaSerializedProtocolSeedV7<C>
where
    C: halo2_proofs::halo2curves::CurveAffine,
{
    protocol: snark_verifier::system::halo2::PlonkProtocol<C>,
    structure_sha256: [u8; 32],
    identity_sha256: [u8; 32],
    proof: Vec<u8>,
    current:
        snark_verifier::pcs::ipa::IpaAccumulator<C, snark_verifier::loader::native::NativeLoader>,
}

#[derive(Clone)]
struct KagemushaSerializedNullParentV7<C>
where
    C: halo2_proofs::halo2curves::CurveAffine,
{
    proof: Vec<u8>,
    current:
        snark_verifier::pcs::ipa::IpaAccumulator<C, snark_verifier::loader::native::NativeLoader>,
    post_proof_fold: KagemushaIpaAccumulationProofV4,
    branch_merge_fold: KagemushaIpaAccumulationProofV4,
}

impl<C> KagemushaSerializedNullParentV7<C>
where
    C: halo2_base::utils::CurveAffineExt,
    C::ScalarExt: halo2_base::utils::ScalarField + From<u64>,
{
    fn parent(&self) -> KagemushaStepParentProofV4<C> {
        KagemushaStepParentProofV4 {
            instances: vec![vec![
                C::ScalarExt::ZERO;
                KAGEMUSHA_SERIALIZED_PUBLIC_INSTANCE_CELLS_V7
            ]],
            proof_bytes: self.proof.clone(),
            carried_lineage: self.current.clone(),
            external_accumulation_proof: self.post_proof_fold.clone(),
        }
    }
}

/// Complete canonical fixed-slot carrier for the final all-zero NullParent.
/// The extra branch folds are required by the fixed two-parent graph even
/// though the two public instance columns are exactly zero.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
struct KagemushaSerializedNullCarrierWireV7 {
    version: u16,
    role: u8,
    manifest_sha256: [u8; 32],
    step_eq_instances: Vec<u128>,
    step_ep_instances: Vec<u128>,
    step_eq_proof_bytes: Vec<u8>,
    step_ep_proof_bytes: Vec<u8>,
    step_eq_current: KagemushaIpaAccumulatorWireV4,
    step_ep_current: KagemushaIpaAccumulatorWireV4,
    step_eq_post_proof_fold: KagemushaIpaAccumulationProofV4,
    step_ep_post_proof_fold: KagemushaIpaAccumulationProofV4,
    step_eq_branch_merge_fold: KagemushaIpaAccumulationProofV4,
    step_ep_branch_merge_fold: KagemushaIpaAccumulationProofV4,
}

impl KagemushaSerializedNullCarrierWireV7 {
    fn validate(
        &self,
        manifest: &super::kagemusha_serialized_audit_v7::KagemushaSerializedAuditManifestV7,
    ) -> Result<(), String> {
        manifest.validate()?;
        let zero_instances = vec![0; KAGEMUSHA_SERIALIZED_PUBLIC_INSTANCE_CELLS_V7];
        if self.version != KAGEMUSHA_SERIALIZED_RELEASE_CARRIER_VERSION_V7
            || self.role != KAGEMUSHA_SERIALIZED_NULL_CARRIER_ROLE_V7
            || self.manifest_sha256 != manifest.sha256()?
            || self.step_eq_instances != zero_instances
            || self.step_ep_instances != zero_instances
            || self.step_eq_proof_bytes.len() != KAGEMUSHA_SERIALIZED_STEP_PROOF_BYTES_V7
            || self.step_ep_proof_bytes.len() != KAGEMUSHA_SERIALIZED_STEP_PROOF_BYTES_V7
            || self.step_eq_proof_bytes.len() > KAGEMUSHA_SERIALIZED_STEP_PROOF_MAX_BYTES_V7
            || self.step_ep_proof_bytes.len() > KAGEMUSHA_SERIALIZED_STEP_PROOF_MAX_BYTES_V7
            || self.step_eq_proof_bytes == self.step_ep_proof_bytes
            || self.step_eq_proof_bytes.len() + self.step_ep_proof_bytes.len()
                != KAGEMUSHA_SERIALIZED_RAW_PROOF_PAIR_BYTES_V7
        {
            return Err("Kagemusha V7 canonical NullParent carrier shape drifted".to_owned());
        }
        self.step_eq_current.to_eq(manifest.k)?;
        self.step_ep_current.to_ep(manifest.k)?;
        self.step_eq_post_proof_fold
            .validate_fixed_transcript(manifest.k)?;
        self.step_ep_post_proof_fold
            .validate_fixed_transcript(manifest.k)?;
        self.step_eq_branch_merge_fold
            .validate_fixed_transcript(manifest.k)?;
        self.step_ep_branch_merge_fold
            .validate_fixed_transcript(manifest.k)?;
        Ok(())
    }
}

struct KagemushaSerializedSealedNullCarrierV7 {
    wire: KagemushaSerializedNullCarrierWireV7,
    canonical_bytes: Vec<u8>,
    canonical_sha256: [u8; 32],
}

fn seal_kagemusha_serialized_null_carrier_v7(
    manifest: &super::kagemusha_serialized_audit_v7::KagemushaSerializedAuditManifestV7,
    step_eq: &KagemushaSerializedNullParentV7<halo2_proofs::halo2curves::pasta::EqAffine>,
    step_ep: &KagemushaSerializedNullParentV7<halo2_proofs::halo2curves::pasta::EpAffine>,
) -> Result<KagemushaSerializedSealedNullCarrierV7, String> {
    let wire = KagemushaSerializedNullCarrierWireV7 {
        version: KAGEMUSHA_SERIALIZED_RELEASE_CARRIER_VERSION_V7,
        role: KAGEMUSHA_SERIALIZED_NULL_CARRIER_ROLE_V7,
        manifest_sha256: manifest.sha256()?,
        step_eq_instances: vec![0; KAGEMUSHA_SERIALIZED_PUBLIC_INSTANCE_CELLS_V7],
        step_ep_instances: vec![0; KAGEMUSHA_SERIALIZED_PUBLIC_INSTANCE_CELLS_V7],
        step_eq_proof_bytes: step_eq.proof.clone(),
        step_ep_proof_bytes: step_ep.proof.clone(),
        step_eq_current: KagemushaIpaAccumulatorWireV4::from_eq(&step_eq.current, manifest.k)?,
        step_ep_current: KagemushaIpaAccumulatorWireV4::from_ep(&step_ep.current, manifest.k)?,
        step_eq_post_proof_fold: step_eq.post_proof_fold.clone(),
        step_ep_post_proof_fold: step_ep.post_proof_fold.clone(),
        step_eq_branch_merge_fold: step_eq.branch_merge_fold.clone(),
        step_ep_branch_merge_fold: step_ep.branch_merge_fold.clone(),
    };
    wire.validate(manifest)?;
    let canonical_bytes = norito::encode_canonical(&wire).map_err(|error| {
        format!("failed to encode Kagemusha V7 canonical NullParent carrier: {error}")
    })?;
    let absolute_max = usize::try_from(KAGEMUSHA_RECURSIVE_SPEND_PROOF_PAIR_ABSOLUTE_MAX_BYTES_V4)
        .map_err(|_| "Kagemusha V7 absolute carrier bound does not fit usize".to_owned())?;
    if canonical_bytes.len() <= KAGEMUSHA_SERIALIZED_RAW_PROOF_PAIR_BYTES_V7
        || canonical_bytes.len() > absolute_max
    {
        return Err(format!(
            "Kagemusha V7 canonical NullParent carrier is {} bytes and violates the existing {absolute_max}-byte absolute payload corridor; release is a hard NO",
            canonical_bytes.len()
        ));
    }
    let decoded: KagemushaSerializedNullCarrierWireV7 = norito::decode_canonical_with_limits(
        &canonical_bytes,
        kagemusha_runtime_norito_decode_limits_v4(canonical_bytes.len()),
    )
    .map_err(|error| {
        if matches!(&error, norito::Error::NonCanonicalEncoding) {
            "Kagemusha V7 NullParent carrier is not canonical Norito".to_owned()
        } else {
            format!("failed to decode Kagemusha V7 canonical NullParent carrier: {error}")
        }
    })?;
    if decoded != wire {
        return Err("Kagemusha V7 canonical NullParent carrier roundtrip drifted".to_owned());
    }
    decoded.validate(manifest)?;
    let canonical_sha256: [u8; 32] = Sha256::digest(&canonical_bytes).into();
    if canonical_sha256 == [0; 32] {
        return Err("Kagemusha V7 canonical NullParent carrier SHA-256 is zero".to_owned());
    }
    Ok(KagemushaSerializedSealedNullCarrierV7 {
        wire,
        canonical_bytes,
        canonical_sha256,
    })
}

fn validate_kagemusha_serialized_null_carrier_seal_v7(
    manifest: &super::kagemusha_serialized_audit_v7::KagemushaSerializedAuditManifestV7,
    carrier: &KagemushaSerializedSealedNullCarrierV7,
) -> Result<(), String> {
    carrier.wire.validate(manifest)?;
    let canonical_bytes = norito::encode_canonical(&carrier.wire).map_err(|error| {
        format!("failed to re-encode Kagemusha V7 canonical NullParent carrier: {error}")
    })?;
    let canonical_sha256: [u8; 32] = Sha256::digest(&canonical_bytes).into();
    let absolute_max = usize::try_from(KAGEMUSHA_RECURSIVE_SPEND_PROOF_PAIR_ABSOLUTE_MAX_BYTES_V4)
        .map_err(|_| "Kagemusha V7 absolute carrier bound does not fit usize".to_owned())?;
    if canonical_bytes != carrier.canonical_bytes
        || canonical_bytes.len() <= KAGEMUSHA_SERIALIZED_RAW_PROOF_PAIR_BYTES_V7
        || canonical_bytes.len() > absolute_max
        || canonical_sha256 != carrier.canonical_sha256
        || canonical_sha256 == [0; 32]
    {
        return Err("Kagemusha V7 canonical NullParent seal/size identity drifted".to_owned());
    }
    Ok(())
}

#[derive(Clone)]
struct KagemushaStepEqNullProtocolCircuitV7 {
    params: KagemushaStepCircuitParamsV7,
}

impl halo2_proofs::plonk::Circuit<Fp> for KagemushaStepEqNullProtocolCircuitV7 {
    type Config = KagemushaStepCompositeConfigV7<Fp>;
    type FloorPlanner = halo2_proofs::circuit::V1;
    type Params = KagemushaStepCircuitParamsV7;

    fn params(&self) -> Self::Params {
        self.params.clone()
    }

    fn without_witnesses(&self) -> Self {
        self.clone()
    }

    fn configure_with_params(
        meta: &mut halo2_proofs::plonk::ConstraintSystem<Fp>,
        params: Self::Params,
    ) -> Self::Config {
        configure_kagemusha_step_eq_composite_v7(meta, &params)
    }

    fn configure(_: &mut halo2_proofs::plonk::ConstraintSystem<Fp>) -> Self::Config {
        unreachable!("Kagemusha V7 Eq null protocol requires authenticated parameters")
    }

    fn synthesize(
        &self,
        config: Self::Config,
        mut layouter: impl halo2_proofs::circuit::Layouter<Fp>,
    ) -> Result<(), halo2_proofs::plonk::Error> {
        let base_params = kagemusha_base_circuit_params_v4(&self.params.base)
            .map_err(|_| halo2_proofs::plonk::Error::Synthesis)?;
        let usable_rows = kagemusha_usable_rows_v4(&self.params.base)
            .map_err(|_| halo2_proofs::plonk::Error::Synthesis)?;
        let builder = halo2_base::gates::circuit::builder::BaseCircuitBuilder::<Fp>::new(false)
            .use_params(base_params);
        <halo2_base::gates::circuit::builder::BaseCircuitBuilder<Fp> as halo2_proofs::plonk::Circuit<
            Fp,
        >>::synthesize(
            &builder,
            config.base,
            layouter.namespace(|| "Kagemusha V7 Eq null Base"),
        )?;
        KagemushaSha256JobsV4::<Fp>::default().synthesize(
            &config.sha,
            &mut layouter,
            &builder.core().copy_manager,
            usable_rows,
        )?;
        KagemushaDenseMsmJobsV5::<halo2_proofs::halo2curves::pasta::EpAffine>::default().synthesize(
            &config.dense,
            &mut layouter,
            &builder.core().copy_manager,
            builder.witness_gen_only(),
            usable_rows,
        )
    }
}

impl KagemushaBootstrapCircuitV1<Fp> for KagemushaStepEqNullProtocolCircuitV7 {
    fn bootstrap_base_circuit_params_v1(
        &self,
    ) -> Result<halo2_base::gates::circuit::BaseCircuitParams, String> {
        kagemusha_base_circuit_params_v4(&self.params.base)
    }
}

#[derive(Clone)]
struct KagemushaStepEpNullProtocolCircuitV7 {
    params: KagemushaStepCircuitParamsV7,
}

impl halo2_proofs::plonk::Circuit<Fq> for KagemushaStepEpNullProtocolCircuitV7 {
    type Config = KagemushaStepCompositeConfigV7<Fq>;
    type FloorPlanner = halo2_proofs::circuit::V1;
    type Params = KagemushaStepCircuitParamsV7;

    fn params(&self) -> Self::Params {
        self.params.clone()
    }

    fn without_witnesses(&self) -> Self {
        self.clone()
    }

    fn configure_with_params(
        meta: &mut halo2_proofs::plonk::ConstraintSystem<Fq>,
        params: Self::Params,
    ) -> Self::Config {
        configure_kagemusha_step_ep_composite_v7(meta, &params)
    }

    fn configure(_: &mut halo2_proofs::plonk::ConstraintSystem<Fq>) -> Self::Config {
        unreachable!("Kagemusha V7 Ep null protocol requires authenticated parameters")
    }

    fn synthesize(
        &self,
        config: Self::Config,
        mut layouter: impl halo2_proofs::circuit::Layouter<Fq>,
    ) -> Result<(), halo2_proofs::plonk::Error> {
        let base_params = kagemusha_base_circuit_params_v4(&self.params.base)
            .map_err(|_| halo2_proofs::plonk::Error::Synthesis)?;
        let usable_rows = kagemusha_usable_rows_v4(&self.params.base)
            .map_err(|_| halo2_proofs::plonk::Error::Synthesis)?;
        let builder = halo2_base::gates::circuit::builder::BaseCircuitBuilder::<Fq>::new(false)
            .use_params(base_params);
        <halo2_base::gates::circuit::builder::BaseCircuitBuilder<Fq> as halo2_proofs::plonk::Circuit<
            Fq,
        >>::synthesize(
            &builder,
            config.base,
            layouter.namespace(|| "Kagemusha V7 Ep null Base"),
        )?;
        KagemushaSha256JobsV4::<Fq>::default().synthesize(
            &config.sha,
            &mut layouter,
            &builder.core().copy_manager,
            usable_rows,
        )?;
        KagemushaDenseMsmJobsV5::<halo2_proofs::halo2curves::pasta::EqAffine>::default().synthesize(
            &config.dense,
            &mut layouter,
            &builder.core().copy_manager,
            builder.witness_gen_only(),
            usable_rows,
        )
    }
}

impl KagemushaBootstrapCircuitV1<Fq> for KagemushaStepEpNullProtocolCircuitV7 {
    fn bootstrap_base_circuit_params_v1(
        &self,
    ) -> Result<halo2_base::gates::circuit::BaseCircuitParams, String> {
        kagemusha_base_circuit_params_v4(&self.params.base)
    }
}

fn kagemusha_serialized_eq_protocol_seed_v7(
    params: &halo2_proofs::poly::ipa::commitment::ParamsIPA<
        halo2_proofs::halo2curves::pasta::EqAffine,
    >,
    circuit_params: &KagemushaStepCircuitParamsV7,
) -> Result<KagemushaSerializedProtocolSeedV7<halo2_proofs::halo2curves::pasta::EqAffine>, String> {
    let target = KagemushaUniversalProtocolTargetV1 {
        base_circuit_params: kagemusha_base_circuit_params_v4(&circuit_params.base)?,
        instance_column_lengths: vec![KAGEMUSHA_SERIALIZED_PUBLIC_INSTANCE_CELLS_V7],
    };
    let circuit = KagemushaStepEqNullProtocolCircuitV7 {
        params: circuit_params.clone(),
    };
    let proving_key = kagemusha_bootstrap_proving_key_v1(params, &target, &circuit)?;
    let instances = vec![vec![
        Fp::ZERO;
        KAGEMUSHA_SERIALIZED_PUBLIC_INSTANCE_CELLS_V7
    ]];
    let (proof, verifying_key) =
        create_augmented_eq_proof_v4(params, proving_key, circuit, &instances)?;
    let expected = usize::try_from(circuit_params.manifest.step_eq_proof_bytes)
        .map_err(|_| "Kagemusha V7 Eq null proof size does not fit usize".to_owned())?;
    if proof.len() != expected {
        return Err("Kagemusha V7 Eq null seed proof size drifted".to_owned());
    }
    let current =
        succinct_verify_step_eq_instances(params, &verifying_key, &proof, &instances, expected)?;
    let protocol = snark_verifier::system::halo2::compile(
        params,
        &verifying_key,
        kagemusha_ipa_compile_config_v4(KAGEMUSHA_SERIALIZED_PUBLIC_INSTANCE_CELLS_V7),
    );
    let structure_sha256 = kagemusha_compiled_protocol_structure_sha256(
        &protocol,
        KagemushaPastaCycleParityV1::StepEq,
    )?;
    let identity_sha256 = kagemusha_compiled_protocol_identity_sha256(
        &protocol,
        KagemushaPastaCycleParityV1::StepEq,
    )?;
    Ok(KagemushaSerializedProtocolSeedV7 {
        protocol,
        structure_sha256,
        identity_sha256,
        proof,
        current,
    })
}

fn kagemusha_serialized_ep_protocol_seed_v7(
    params: &halo2_proofs::poly::ipa::commitment::ParamsIPA<
        halo2_proofs::halo2curves::pasta::EpAffine,
    >,
    circuit_params: &KagemushaStepCircuitParamsV7,
) -> Result<KagemushaSerializedProtocolSeedV7<halo2_proofs::halo2curves::pasta::EpAffine>, String> {
    let target = KagemushaUniversalProtocolTargetV1 {
        base_circuit_params: kagemusha_base_circuit_params_v4(&circuit_params.base)?,
        instance_column_lengths: vec![KAGEMUSHA_SERIALIZED_PUBLIC_INSTANCE_CELLS_V7],
    };
    let circuit = KagemushaStepEpNullProtocolCircuitV7 {
        params: circuit_params.clone(),
    };
    let proving_key = kagemusha_bootstrap_proving_key_v1(params, &target, &circuit)?;
    let instances = vec![vec![
        Fq::ZERO;
        KAGEMUSHA_SERIALIZED_PUBLIC_INSTANCE_CELLS_V7
    ]];
    let (proof, verifying_key) =
        create_augmented_ep_proof_v4(params, proving_key, circuit, &instances)?;
    let expected = usize::try_from(circuit_params.manifest.step_ep_proof_bytes)
        .map_err(|_| "Kagemusha V7 Ep null proof size does not fit usize".to_owned())?;
    if proof.len() != expected {
        return Err("Kagemusha V7 Ep null seed proof size drifted".to_owned());
    }
    let current =
        succinct_verify_step_ep_instances(params, &verifying_key, &proof, &instances, expected)?;
    let protocol = snark_verifier::system::halo2::compile(
        params,
        &verifying_key,
        kagemusha_ipa_compile_config_v4(KAGEMUSHA_SERIALIZED_PUBLIC_INSTANCE_CELLS_V7),
    );
    let structure_sha256 = kagemusha_compiled_protocol_structure_sha256(
        &protocol,
        KagemushaPastaCycleParityV1::StepEp,
    )?;
    let identity_sha256 = kagemusha_compiled_protocol_identity_sha256(
        &protocol,
        KagemushaPastaCycleParityV1::StepEp,
    )?;
    Ok(KagemushaSerializedProtocolSeedV7 {
        protocol,
        structure_sha256,
        identity_sha256,
        proof,
        current,
    })
}

fn kagemusha_serialized_eq_null_from_seed_v7(
    params: &halo2_proofs::poly::ipa::commitment::ParamsIPA<
        halo2_proofs::halo2curves::pasta::EqAffine,
    >,
    seed: &KagemushaSerializedProtocolSeedV7<halo2_proofs::halo2curves::pasta::EqAffine>,
    k: u32,
) -> Result<KagemushaSerializedNullParentV7<halo2_proofs::halo2curves::pasta::EqAffine>, String> {
    let (post_proof_fold, _) = super::kagemusha_accumulation::fold_eq_accumulators_v4(
        params,
        k,
        seed.current.clone(),
        Some(seed.current.clone()),
    )?;
    let (branch_merge_fold, _) = super::kagemusha_accumulation::fold_eq_accumulators_v4(
        params,
        k,
        seed.current.clone(),
        Some(seed.current.clone()),
    )?;
    Ok(KagemushaSerializedNullParentV7 {
        proof: seed.proof.clone(),
        current: seed.current.clone(),
        post_proof_fold,
        branch_merge_fold,
    })
}

fn kagemusha_serialized_ep_null_from_seed_v7(
    params: &halo2_proofs::poly::ipa::commitment::ParamsIPA<
        halo2_proofs::halo2curves::pasta::EpAffine,
    >,
    seed: &KagemushaSerializedProtocolSeedV7<halo2_proofs::halo2curves::pasta::EpAffine>,
    k: u32,
) -> Result<KagemushaSerializedNullParentV7<halo2_proofs::halo2curves::pasta::EpAffine>, String> {
    let (post_proof_fold, _) = super::kagemusha_accumulation::fold_ep_accumulators_v4(
        params,
        k,
        seed.current.clone(),
        Some(seed.current.clone()),
    )?;
    let (branch_merge_fold, _) = super::kagemusha_accumulation::fold_ep_accumulators_v4(
        params,
        k,
        seed.current.clone(),
        Some(seed.current.clone()),
    )?;
    Ok(KagemushaSerializedNullParentV7 {
        proof: seed.proof.clone(),
        current: seed.current.clone(),
        post_proof_fold,
        branch_merge_fold,
    })
}

fn kagemusha_serialized_eq_recursion_from_null_v7(
    params: &halo2_proofs::poly::ipa::commitment::ParamsIPA<
        halo2_proofs::halo2curves::pasta::EqAffine,
    >,
    protocol: snark_verifier::system::halo2::PlonkProtocol<
        halo2_proofs::halo2curves::pasta::EqAffine,
    >,
    structure_sha256: [u8; 32],
    null: &KagemushaSerializedNullParentV7<halo2_proofs::halo2curves::pasta::EqAffine>,
) -> Result<KagemushaStepParityRecursionV4<halo2_proofs::halo2curves::pasta::EqAffine>, String> {
    Ok(KagemushaStepParityRecursionV4 {
        succinct_vk: kagemusha_eq_succinct_vk_v4(params)?,
        compiled_parent_protocol: protocol,
        fixed_structure_sha256: structure_sha256,
        parents: [null.parent(), null.parent()],
        branch_merge_fold: null.branch_merge_fold.clone(),
    })
}

fn kagemusha_serialized_ep_recursion_from_null_v7(
    params: &halo2_proofs::poly::ipa::commitment::ParamsIPA<
        halo2_proofs::halo2curves::pasta::EpAffine,
    >,
    protocol: snark_verifier::system::halo2::PlonkProtocol<
        halo2_proofs::halo2curves::pasta::EpAffine,
    >,
    structure_sha256: [u8; 32],
    null: &KagemushaSerializedNullParentV7<halo2_proofs::halo2curves::pasta::EpAffine>,
) -> Result<KagemushaStepParityRecursionV4<halo2_proofs::halo2curves::pasta::EpAffine>, String> {
    Ok(KagemushaStepParityRecursionV4 {
        succinct_vk: kagemusha_ep_succinct_vk_v4(params)?,
        compiled_parent_protocol: protocol,
        fixed_structure_sha256: structure_sha256,
        parents: [null.parent(), null.parent()],
        branch_merge_fold: null.branch_merge_fold.clone(),
    })
}

fn collect_kagemusha_serialized_native_audit_v7<C>(
    public_inputs: &KagemushaSerializedPublicInputsV7<'_>,
    proof_step_count: u32,
    params: &KagemushaStepCircuitParamsV7,
    recursion: &KagemushaStepParityRecursionV4<C>,
) -> Result<(KagemushaScalarAuditOutputV4<C>, Vec<C::ScalarExt>), String>
where
    C: KagemushaSerializedAuditCurveV7,
    C::Base: halo2_base::utils::BigPrimeField,
    C::ScalarExt: super::kagemusha_serialized_audit_v7::KagemushaSerializedAuditFieldV7
        + halo2_base::utils::ScalarField
        + ff::FromUniformBytes<64>,
{
    let base_layout = public_inputs
        .core
        .validate_for_audit_derivation_prepass(proof_step_count, &params.base)?;
    let layout = KagemushaSerializedPublicLayoutV7::for_k17(&base_layout)?
        .recursive_parent_layout(&base_layout);
    let mut builder =
        halo2_base::gates::circuit::builder::BaseCircuitBuilder::<C::ScalarExt>::new(true)
            .use_params(kagemusha_base_circuit_params_v4(&params.base)?);
    let values =
        public_inputs.instance_column::<C::ScalarExt>(proof_step_count, &params.base, C::PARITY)?;
    let public_cells = builder.main(0).assign_witnesses(values);
    builder.assigned_instances = vec![public_cells.clone()];
    let (count, rank) = match C::PARITY {
        KagemushaPastaCycleParityV1::StepEq => (
            params.manifest.eq_coefficient_count,
            params.manifest.step_eq_phase_zero_rank,
        ),
        KagemushaPastaCycleParityV1::StepEp => (
            params.manifest.ep_coefficient_count,
            params.manifest.step_ep_phase_zero_rank,
        ),
    };
    let profile = super::kagemusha_serialized_audit_v7::KagemushaSerializedAuditProfileV7::for_captured_coefficient_count(
        usize::try_from(count)
            .map_err(|_| "Kagemusha V7 native coefficient count does not fit usize".to_owned())?,
    )?;
    let mut sha_jobs = KagemushaSha256JobsV4::default();
    let assigned = assign_kagemusha_parity_native_vector_v7::<C>(
        &mut builder,
        &mut sha_jobs,
        &public_cells,
        &params.base,
        &layout,
        recursion,
        profile,
        usize::try_from(rank)
            .map_err(|_| "Kagemusha V7 phase-zero rank does not fit usize".to_owned())?,
    )?;
    let coefficients = assigned
        .vector
        .coefficients()
        .iter()
        .map(|cell| cell.value.evaluate())
        .collect::<Vec<_>>();
    let canonical =
        kagemusha_canonical_audit_polynomial_v7(&assigned.output, public_inputs.core.parent_count)?;
    if coefficients.len() != profile.coefficient_count() || coefficients != canonical {
        return Err("Kagemusha V7 native prepass coefficient count drifted".to_owned());
    }
    halo2_proofs::release_allocator_slack();
    Ok((assigned.output, canonical))
}

fn kagemusha_serialized_placeholder_join_v7()
-> super::kagemusha_serialized_audit_v7::KagemushaSerializedAuditPublicJoinV7 {
    use super::kagemusha_serialized_audit_v7::kagemusha_serialized_bytes_to_chunks_v7;
    use halo2_proofs::halo2curves::{
        group::GroupEncoding as _,
        group::prime::PrimeCurveAffine as _,
        pasta::{EpAffine, EqAffine},
    };

    super::kagemusha_serialized_audit_v7::KagemushaSerializedAuditPublicJoinV7 {
        step_eq_commitment: kagemusha_serialized_bytes_to_chunks_v7(
            EqAffine::generator()
                .to_bytes()
                .as_ref()
                .try_into()
                .expect("Pasta Eq encoding is 32 bytes"),
        ),
        step_ep_commitment: kagemusha_serialized_bytes_to_chunks_v7(
            EpAffine::generator()
                .to_bytes()
                .as_ref()
                .try_into()
                .expect("Pasta Ep encoding is 32 bytes"),
        ),
        challenge: [2, 0],
        eq_evaluation: [0, 0],
        ep_evaluation: [0, 0],
    }
}

fn kagemusha_serialized_parent_digest_context_v7(
    manifest: &super::kagemusha_serialized_audit_v7::KagemushaSerializedAuditManifestV7,
) -> Result<super::kagemusha_serialized_audit_v7::KagemushaSerializedParentDigestContextV7, String>
{
    manifest.validate()?;
    Ok(
        super::kagemusha_serialized_audit_v7::KagemushaSerializedParentDigestContextV7 {
            profile_sha256: manifest.sha256()?,
            step_eq_vk_sha256: manifest.step_eq_vk_sha256,
            step_ep_vk_sha256: manifest.step_ep_vk_sha256,
            eq_coefficient_count: manifest.eq_coefficient_count,
            ep_coefficient_count: manifest.ep_coefficient_count,
        },
    )
}

fn kagemusha_serialized_frozen_statement_v7(
    public_inputs: &KagemushaPastaCyclePublicInputsV4,
    proof_step_count: u32,
) -> [u8; 32] {
    let header = public_inputs.compact_header_chunks_v5(proof_step_count);
    super::kagemusha_serialized_audit_v7::kagemusha_serialized_exact_chunks_to_bytes_v7(
        header[KAGEMUSHA_COMPACT_STATEMENT_DIGEST_OFFSET_V5
            ..KAGEMUSHA_COMPACT_STATEMENT_DIGEST_OFFSET_V5 + 2]
            .try_into()
            .expect("V7 frozen statement has two chunks"),
    )
}

fn kagemusha_rebind_calibration_manifest_v7(
    manifest: &super::kagemusha_serialized_audit_v7::KagemushaSerializedAuditManifestV7,
) -> Result<KagemushaGenerationCalibrationV4, String> {
    let mut calibration = kagemusha_generation_calibration_v4(
        manifest.step_eq_vk_sha256,
        manifest.step_ep_vk_sha256,
    )?;
    let manifest_sha256 = manifest.sha256()?;
    let manifest_limbs = kagemusha_calibration_exact_limbs_v4(manifest_sha256);
    calibration.public_inputs.manifest_sha256 = manifest_limbs;
    calibration.public_inputs.result_state[super::kagemusha_v2::S_ARTIFACT_MANIFEST_SHA256
        ..super::kagemusha_v2::S_ARTIFACT_MANIFEST_SHA256 + 8]
        .copy_from_slice(&manifest_limbs);
    let mut operation = calibration.public_inputs.operation.to_fields()?;
    kagemusha_calibration_put_digest_v4(
        &mut operation,
        super::kagemusha_v2::I_ARTIFACT_MANIFEST_SHA256,
        manifest_sha256,
    )?;
    calibration.public_inputs.operation = KagemushaStepOperationVectorV4::from_fields(operation);
    calibration.public_inputs.step_eq_compiled_protocol_sha256 =
        kagemusha_sha256_public_words(manifest.step_eq_vk_sha256);
    calibration.public_inputs.step_ep_compiled_protocol_sha256 =
        kagemusha_sha256_public_words(manifest.step_ep_vk_sha256);
    Ok(calibration)
}

const KAGEMUSHA_SERIALIZED_FIXTURE_AMOUNT_V7: u128 = 2;
const KAGEMUSHA_SERIALIZED_FIXTURE_PAYER_V7: &str = "kagemusha-v7-release-proof-payer";
const KAGEMUSHA_SERIALIZED_FIXTURE_SPEND_KEY_V7: [u8; 32] = [0x71; 32];
const KAGEMUSHA_SERIALIZED_FIXTURE_INIT_RHO_V7: [u8; 32] = [0x72; 32];
const KAGEMUSHA_SERIALIZED_FIXTURE_INIT_OPERATION_V7: [u8; 32] = [0x73; 32];
const KAGEMUSHA_SERIALIZED_FIXTURE_BRANCH_SPEND_KEY_V7: [u8; 32] = [0x75; 32];

struct KagemushaSerializedInitializationRelationV7 {
    topup: super::confidential_v2::KagemushaTopUpShieldPublicInputsV2,
    secure: super::confidential_v2::KagemushaStepSecureWitnessV3,
    output_membership: super::kagemusha_v2::KagemushaOutputMembershipWitnessV4,
    diversifier: [u8; 32],
}

#[allow(clippy::too_many_lines)]
fn kagemusha_serialized_initialization_relation_v7(
    network_id: &iroha_data_model::NetworkId,
    asset_definition_id: &str,
    asset_scale: u32,
) -> Result<KagemushaSerializedInitializationRelationV7, String> {
    use super::{confidential_v2, kagemusha_v2};

    let diversifier = confidential_v2::derive_confidential_diversifier_v2(
        b"iroha:kagemusha:v7:release-proof:init-diversifier",
    );
    let empty_path = confidential_v2::compute_confidential_merkle_path_v3(&[], 0)?;
    let secure = confidential_v2::prepare_kagemusha_step_topup_witness_v3(
        network_id,
        asset_definition_id,
        KAGEMUSHA_SERIALIZED_FIXTURE_PAYER_V7,
        KAGEMUSHA_SERIALIZED_FIXTURE_INIT_OPERATION_V7,
        KAGEMUSHA_SERIALIZED_FIXTURE_AMOUNT_V7,
        asset_scale,
        &KAGEMUSHA_SERIALIZED_FIXTURE_SPEND_KEY_V7,
        KAGEMUSHA_SERIALIZED_FIXTURE_INIT_RHO_V7,
        diversifier,
        0,
        &empty_path,
    )?;
    let asset_tag = confidential_v2::derive_confidential_asset_tag_v3(asset_definition_id)?;
    let network_tag = confidential_v2::derive_confidential_network_tag_v3(network_id)?;
    let payer_tag = confidential_v2::derive_kagemusha_topup_payer_tag_v3(
        KAGEMUSHA_SERIALIZED_FIXTURE_PAYER_V7,
    )?;
    let operation_tag = confidential_v2::derive_kagemusha_topup_operation_tag_v3(
        &KAGEMUSHA_SERIALIZED_FIXTURE_INIT_OPERATION_V7,
    )?;
    let owner_tag = confidential_v2::derive_confidential_owner_tag_v3_with_diversifier(
        &KAGEMUSHA_SERIALIZED_FIXTURE_SPEND_KEY_V7,
        diversifier,
    )?;
    let output_commitment = confidential_v2::derive_confidential_note_v3(
        asset_tag,
        KAGEMUSHA_SERIALIZED_FIXTURE_AMOUNT_V7,
        KAGEMUSHA_SERIALIZED_FIXTURE_INIT_RHO_V7,
        owner_tag,
    )?;
    let spend_nullifier = confidential_v2::derive_confidential_nullifier_v3(
        &KAGEMUSHA_SERIALIZED_FIXTURE_SPEND_KEY_V7,
        KAGEMUSHA_SERIALIZED_FIXTURE_INIT_RHO_V7,
        asset_tag,
        network_tag,
    )?;
    let initial_root = confidential_v2::compute_confidential_root_v3(&[])?;
    let final_commitments = [output_commitment];
    let final_root = confidential_v2::compute_confidential_root_v3(&final_commitments)?;
    if empty_path.root != initial_root {
        return Err("Kagemusha V7 initialization empty path/root mismatch".to_owned());
    }
    let output_membership = kagemusha_v2::KagemushaOutputMembershipWitnessV4 {
        operation: kagemusha_v2::KagemushaOutputMembershipOperationV4::Init,
        initial_root,
        final_root,
        recipient: Some(kagemusha_v2::KagemushaOutputMembershipLeafV4 {
            commitment: output_commitment,
            leaf_index: 0,
            update_path: kagemusha_calibration_membership_path_v4(empty_path),
            membership_path: kagemusha_calibration_membership_path_v4(
                confidential_v2::compute_confidential_merkle_path_v3(&final_commitments, 0)?,
            ),
        }),
        change: None,
        dummy_leaf_index: 1,
        dummy_path: kagemusha_calibration_membership_path_v4(
            confidential_v2::compute_confidential_merkle_path_v3(&final_commitments, 1)?,
        ),
    };
    kagemusha_v2::KagemushaOutputMembershipCircuitV4::new(output_membership.clone())?;
    let topup = confidential_v2::KagemushaTopUpShieldPublicInputsV2 {
        output_commitment,
        spend_nullifier,
        initial_root,
        finalized_root: final_root,
        atomic_amount: iroha_data_model::offline::kagemusha_confidential_amount_encoding_v2(
            KAGEMUSHA_SERIALIZED_FIXTURE_AMOUNT_V7,
        ),
        asset_scale: {
            let mut encoded = [0; 32];
            encoded[..4].copy_from_slice(&asset_scale.to_le_bytes());
            encoded
        },
        leaf_index: [0; 32],
        asset_tag,
        network_tag,
        payer_tag,
        operation_tag,
    };
    Ok(KagemushaSerializedInitializationRelationV7 {
        topup,
        secure,
        output_membership,
        diversifier,
    })
}

struct KagemushaSerializedInitMaterialV7 {
    relation: KagemushaSerializedInitializationRelationV7,
    statement: KagemushaRecursiveSpendPublicStatementV4,
    public_inputs: KagemushaPastaCyclePublicInputsV4,
}

#[allow(clippy::too_many_lines)]
fn kagemusha_serialized_init_material_v7(
    material: &KagemushaSerializedReleaseMaterialV7,
) -> Result<KagemushaSerializedInitMaterialV7, String> {
    use iroha_data_model::{
        NetworkId,
        asset::AssetDefinitionId,
        domain::DomainId,
        offline::{
            KAGEMUSHA_RECURSIVE_SPEND_WIRE_VERSION_V4, KagemushaPastaCycleParityV1,
            KagemushaRecursiveSpendArtifactBindingV4, KagemushaRecursiveSpendBranchClaimV2,
            KagemushaRecursiveSpendTopUpAnchorRefV2, KagemushaScaledAmountV2,
            KagemushaSpendableNoteDescriptorV2, kagemusha_recursive_spend_verifier_key_id_v4,
        },
    };

    let network_id = NetworkId::from_genesis_hash(iroha_crypto::HashOf::<
        iroha_data_model::block::BlockHeader,
    >::from_untyped_unchecked(
        iroha_crypto::Hash::new(b"kagemusha-v7-release-proof-network"),
    ));
    let domain = DomainId::try_new("kagemusha", "internal")
        .map_err(|error| format!("failed to construct Kagemusha V7 fixture domain: {error}"))?;
    let asset = AssetDefinitionId::derive_from_components(
        domain,
        "serialized"
            .parse()
            .map_err(|error| format!("failed to construct Kagemusha V7 fixture asset: {error}"))?,
    );
    let asset_scale = 0;
    let relation = kagemusha_serialized_initialization_relation_v7(
        &network_id,
        &asset.to_string(),
        asset_scale,
    )?;
    let manifest_sha256 = material.manifest.sha256()?;
    let anchor_digest = [0x74; 32];
    let anchor_ref = KagemushaRecursiveSpendTopUpAnchorRefV2 {
        topup_operation_id: KAGEMUSHA_SERIALIZED_FIXTURE_INIT_OPERATION_V7,
        anchor_digest,
    };
    anchor_ref.validate().map_err(|error| error.to_string())?;
    let amount = KagemushaScaledAmountV2::new(KAGEMUSHA_SERIALIZED_FIXTURE_AMOUNT_V7, asset_scale)
        .map_err(|error| error.to_string())?;
    let artifact_binding = KagemushaRecursiveSpendArtifactBindingV4 {
        version: KAGEMUSHA_RECURSIVE_SPEND_WIRE_VERSION_V4,
        generation: "serialized-v7-release-proof".to_owned(),
        manifest_sha256,
    };
    let statement = KagemushaRecursiveSpendPublicStatementV4 {
        network_id,
        asset: asset.clone(),
        asset_scale,
        final_root: relation.topup.finalized_root,
        next_zero_leaf_index: relation.output_membership.dummy_leaf_index,
        topup_anchor_refs: vec![anchor_ref],
        proof_step_count: 1,
        peer_hop_count: 0,
        current_note: KagemushaSpendableNoteDescriptorV2 {
            network_id,
            asset,
            note_commitment: relation.topup.output_commitment,
            spend_nullifier: relation.topup.spend_nullifier,
            amount,
        },
        branch_claims: vec![
            KagemushaRecursiveSpendBranchClaimV2::root(anchor_digest)
                .map_err(|error| error.to_string())?,
        ],
        transition: None,
        verifier_key_id: kagemusha_recursive_spend_verifier_key_id_v4(
            KagemushaPastaCycleParityV1::StepEq,
            manifest_sha256,
        ),
        artifact_binding,
    };
    statement
        .validate_public_binding()
        .map_err(|error| error.to_string())?;
    let operation = KagemushaStepOperationVectorV4::from_candidate_qualification_init_v4(
        &statement,
        &relation.topup,
        &relation.output_membership,
        KAGEMUSHA_SERIALIZED_FIXTURE_PAYER_V7,
    )?;
    let public_inputs = super::kagemusha_v2::kagemusha_public_inputs_for_statement_v4(
        &statement,
        operation,
        manifest_sha256,
        material.manifest.step_eq_vk_sha256,
        material.manifest.step_ep_vk_sha256,
    )?;
    if public_inputs.parent_count != 0
        || public_inputs.result_state
            != super::kagemusha_v2::KagemushaRecursiveSpendStateVectorV5::from_statement_v4(
                &statement,
            )?
            .limbs
            .to_vec()
    {
        return Err("Kagemusha V7 initialization public state is not canonical".to_owned());
    }
    Ok(KagemushaSerializedInitMaterialV7 {
        relation,
        statement,
        public_inputs,
    })
}

struct KagemushaSerializedNoteOpeningV7 {
    rho: [u8; 32],
    diversifier: [u8; 32],
    leaf_index: usize,
    path: super::confidential_v2::ConfidentialMerklePathV2,
}

struct KagemushaSerializedSiblingBranchV7 {
    label: &'static str,
    statement: KagemushaRecursiveSpendPublicStatementV4,
    public_inputs: KagemushaPastaCyclePublicInputsV4,
    opening: KagemushaSerializedNoteOpeningV7,
}

struct KagemushaSerializedSiblingMaterialV7 {
    branches: Vec<KagemushaSerializedSiblingBranchV7>,
    secure: super::confidential_v2::KagemushaStepSecureWitnessV3,
    output_membership: super::kagemusha_v2::KagemushaOutputMembershipWitnessV4,
    next_zero_path: super::confidential_v2::ConfidentialMerklePathV2,
}

#[allow(clippy::too_many_lines)]
fn kagemusha_serialized_sibling_material_v7(
    material: &KagemushaSerializedReleaseMaterialV7,
    init: &KagemushaSerializedInitMaterialV7,
    base_bundle_digest: [u8; 32],
) -> Result<KagemushaSerializedSiblingMaterialV7, String> {
    use super::{confidential_v2, kagemusha_v2};
    use iroha_data_model::offline::{
        KagemushaRecursiveSpendBranchV2, KagemushaRecursiveSpendInputBranchV2,
        KagemushaRecursiveSpendSplitIntentV4, KagemushaScaledAmountV2,
        KagemushaSpendableNoteDescriptorV2,
    };

    if base_bundle_digest == [0; 32] {
        return Err("Kagemusha V7 base carrier digest is zero".to_owned());
    }
    let input_leaf = init
        .relation
        .output_membership
        .recipient
        .as_ref()
        .ok_or_else(|| "Kagemusha V7 initialization has no output leaf".to_owned())?;
    let input_path = confidential_v2::validate_confidential_membership_path_v3(
        input_leaf.commitment,
        usize::try_from(input_leaf.leaf_index)
            .map_err(|_| "Kagemusha V7 init leaf index does not fit usize")?,
        &kagemusha_candidate_private_path_v4(&input_leaf.membership_path),
    )?;
    let next_zero_leaf_index = usize::try_from(init.relation.output_membership.dummy_leaf_index)
        .map_err(|_| "Kagemusha V7 init frontier does not fit usize")?;
    let next_zero_path = confidential_v2::validate_confidential_next_zero_path_v3(
        next_zero_leaf_index,
        &kagemusha_candidate_private_path_v4(&init.relation.output_membership.dummy_path),
    )?;
    let recipient_rho = [0x76; 32];
    let change_rho = [0x77; 32];
    let recipient_diversifier = confidential_v2::derive_confidential_diversifier_v2(
        b"iroha:kagemusha:v7:release-proof:recipient",
    );
    let change_diversifier = confidential_v2::derive_confidential_diversifier_v2(
        b"iroha:kagemusha:v7:release-proof:change",
    );
    if recipient_diversifier == change_diversifier || recipient_rho == change_rho {
        return Err("Kagemusha V7 sibling note openings collide".to_owned());
    }
    let recipient_owner = confidential_v2::derive_confidential_owner_tag_v3_with_diversifier(
        &KAGEMUSHA_SERIALIZED_FIXTURE_BRANCH_SPEND_KEY_V7,
        recipient_diversifier,
    )?;
    let change_owner = confidential_v2::derive_confidential_owner_tag_v3_with_diversifier(
        &KAGEMUSHA_SERIALIZED_FIXTURE_BRANCH_SPEND_KEY_V7,
        change_diversifier,
    )?;
    let recipient_commitment = confidential_v2::derive_confidential_note_v3(
        init.relation.topup.asset_tag,
        1,
        recipient_rho,
        recipient_owner,
    )?;
    let change_commitment = confidential_v2::derive_confidential_note_v3(
        init.relation.topup.asset_tag,
        1,
        change_rho,
        change_owner,
    )?;
    let recipient_nullifier = confidential_v2::derive_confidential_nullifier_v3(
        &KAGEMUSHA_SERIALIZED_FIXTURE_BRANCH_SPEND_KEY_V7,
        recipient_rho,
        init.relation.topup.asset_tag,
        init.relation.topup.network_tag,
    )?;
    let change_nullifier = confidential_v2::derive_confidential_nullifier_v3(
        &KAGEMUSHA_SERIALIZED_FIXTURE_BRANCH_SPEND_KEY_V7,
        change_rho,
        init.relation.topup.asset_tag,
        init.relation.topup.network_tag,
    )?;
    let append_paths = confidential_v2::derive_confidential_sequential_append_paths_v3(
        next_zero_leaf_index,
        &next_zero_path,
        &[recipient_commitment, change_commitment],
    )?;
    let [recipient_paths, change_paths] = append_paths.leaves.as_slice() else {
        return Err("Kagemusha V7 sibling split did not derive two outputs".to_owned());
    };
    let expected_change_leaf = next_zero_leaf_index
        .checked_add(1)
        .ok_or_else(|| "Kagemusha V7 sibling change index overflowed".to_owned())?;
    let expected_next_zero = next_zero_leaf_index
        .checked_add(2)
        .ok_or_else(|| "Kagemusha V7 sibling frontier overflowed".to_owned())?;
    if append_paths.initial_root != init.statement.final_root
        || recipient_paths.leaf_index != next_zero_leaf_index
        || change_paths.leaf_index != expected_change_leaf
        || append_paths.next_zero_leaf_index != expected_next_zero
    {
        return Err("Kagemusha V7 sibling split append topology drifted".to_owned());
    }
    let output_membership = kagemusha_v2::KagemushaOutputMembershipWitnessV4 {
        operation: kagemusha_v2::KagemushaOutputMembershipOperationV4::Split,
        initial_root: append_paths.initial_root,
        final_root: append_paths.final_root,
        recipient: Some(kagemusha_v2::KagemushaOutputMembershipLeafV4 {
            commitment: recipient_commitment,
            leaf_index: u32::try_from(recipient_paths.leaf_index)
                .map_err(|_| "Kagemusha V7 recipient leaf index does not fit u32")?,
            update_path: kagemusha_calibration_membership_path_v4(
                recipient_paths.update_path.clone(),
            ),
            membership_path: kagemusha_calibration_membership_path_v4(
                recipient_paths.membership_path.clone(),
            ),
        }),
        change: Some(kagemusha_v2::KagemushaOutputMembershipLeafV4 {
            commitment: change_commitment,
            leaf_index: u32::try_from(change_paths.leaf_index)
                .map_err(|_| "Kagemusha V7 change leaf index does not fit u32")?,
            update_path: kagemusha_calibration_membership_path_v4(change_paths.update_path.clone()),
            membership_path: kagemusha_calibration_membership_path_v4(
                change_paths.membership_path.clone(),
            ),
        }),
        dummy_leaf_index: u32::try_from(append_paths.next_zero_leaf_index)
            .map_err(|_| "Kagemusha V7 sibling frontier does not fit u32")?,
        dummy_path: kagemusha_calibration_membership_path_v4(append_paths.next_zero_path.clone()),
    };
    kagemusha_v2::KagemushaOutputMembershipCircuitV4::new(output_membership.clone())?;
    let amount_one = KagemushaScaledAmountV2::new(1, init.statement.asset_scale)
        .map_err(|error| error.to_string())?;
    let recipient_note = KagemushaSpendableNoteDescriptorV2 {
        network_id: init.statement.network_id,
        asset: init.statement.asset.clone(),
        note_commitment: recipient_commitment,
        spend_nullifier: recipient_nullifier,
        amount: amount_one,
    };
    let change_note = KagemushaSpendableNoteDescriptorV2 {
        network_id: init.statement.network_id,
        asset: init.statement.asset.clone(),
        note_commitment: change_commitment,
        spend_nullifier: change_nullifier,
        amount: amount_one,
    };
    let split = KagemushaRecursiveSpendSplitIntentV4 {
        network_id: init.statement.network_id,
        asset: init.statement.asset.clone(),
        inputs: vec![KagemushaRecursiveSpendInputBranchV2 {
            bundle_digest: base_bundle_digest,
            input_note: init.statement.current_note.clone(),
            branch_claims: init.statement.branch_claims.clone(),
            input_root: init.statement.final_root,
            proof_step_count: init.statement.proof_step_count,
            peer_hop_count: init.statement.peer_hop_count,
        }],
        topup_anchor_refs: init.statement.topup_anchor_refs.clone(),
        asset_scale: init.statement.asset_scale,
        output_artifact_binding: init.statement.artifact_binding.clone(),
        transfer_amount: amount_one,
        recipient_output: recipient_note,
        change_output: Some(change_note),
        recipient_request_digest: [0x78; 32],
        operation_id: [0x79; 32],
    };
    let change_amount = split
        .change_output
        .as_ref()
        .ok_or_else(|| "Kagemusha V7 sibling split omitted change".to_owned())?
        .amount
        .atomic_units;
    if split.inputs[0].input_note.amount.atomic_units
        != split
            .recipient_output
            .amount
            .atomic_units
            .checked_add(change_amount)
            .ok_or_else(|| "Kagemusha V7 sibling conservation overflowed".to_owned())?
        || split.transfer_amount.atomic_units != 1
    {
        return Err("Kagemusha V7 sibling split does not conserve 2 into 1 + 1".to_owned());
    }
    split
        .validate_public_binding()
        .map_err(|error| error.to_string())?;
    let recipient_statement = kagemusha_v2::kagemusha_recursive_spend_append_statement_v4(
        &split,
        KagemushaRecursiveSpendBranchV2::Recipient,
        output_membership.final_root,
        output_membership.dummy_leaf_index,
    )?;
    let change_statement = kagemusha_v2::kagemusha_recursive_spend_append_statement_v4(
        &split,
        KagemushaRecursiveSpendBranchV2::Change,
        output_membership.final_root,
        output_membership.dummy_leaf_index,
    )?;
    let transfer_public = super::kagemusha_step_transition::KagemushaStepTransferPublicV4 {
        input_commitments: [init.statement.current_note.note_commitment, [0; 32]],
        input_nullifiers: [init.statement.current_note.spend_nullifier, [0; 32]],
        output_commitments: [recipient_commitment, change_commitment],
        root: init.statement.final_root,
        asset_tag: init.relation.topup.asset_tag,
        network_tag: init.relation.topup.network_tag,
    };
    let recipient_operation = KagemushaStepOperationVectorV4::from_append_v4(
        &split,
        &recipient_statement,
        &transfer_public,
        &output_membership,
    )?;
    let change_operation = KagemushaStepOperationVectorV4::from_append_v4(
        &split,
        &change_statement,
        &transfer_public,
        &output_membership,
    )?;
    let secure = confidential_v2::prepare_kagemusha_step_transfer_witness_v3_with_paths(
        &init.statement.network_id,
        &init.statement.asset.to_string(),
        &KAGEMUSHA_SERIALIZED_FIXTURE_SPEND_KEY_V7,
        &[input_path, next_zero_path],
        &[confidential_v2::ConfidentialTransferInputV2 {
            amount: KAGEMUSHA_SERIALIZED_FIXTURE_AMOUNT_V7,
            rho: KAGEMUSHA_SERIALIZED_FIXTURE_INIT_RHO_V7,
            diversifier: init.relation.diversifier,
            leaf_index: 0,
        }],
        &[
            confidential_v2::ConfidentialTransferOutputV2 {
                amount: 1,
                rho: recipient_rho,
                owner_tag: recipient_owner,
            },
            confidential_v2::ConfidentialTransferOutputV2 {
                amount: 1,
                rho: change_rho,
                owner_tag: change_owner,
            },
        ],
        init.statement.final_root,
    )?;
    let manifest_sha256 = material.manifest.sha256()?;
    let mut recipient_public = kagemusha_v2::kagemusha_public_inputs_for_statement_v4(
        &recipient_statement,
        recipient_operation,
        manifest_sha256,
        material.manifest.step_eq_vk_sha256,
        material.manifest.step_ep_vk_sha256,
    )?;
    let mut change_public = kagemusha_v2::kagemusha_public_inputs_for_statement_v4(
        &change_statement,
        change_operation,
        manifest_sha256,
        material.manifest.step_eq_vk_sha256,
        material.manifest.step_ep_vk_sha256,
    )?;
    for public in [&mut recipient_public, &mut change_public] {
        public.parent_count = 1;
        public.parent_states[0] = init.public_inputs.result_state.clone();
    }
    if recipient_statement.current_note == change_statement.current_note
        || recipient_statement
            .digest()
            .map_err(|error| error.to_string())?
            == change_statement
                .digest()
                .map_err(|error| error.to_string())?
        || recipient_public.operation == change_public.operation
        || recipient_public.result_state == change_public.result_state
    {
        return Err("Kagemusha V7 sibling semantics are not distinct".to_owned());
    }
    Ok(KagemushaSerializedSiblingMaterialV7 {
        branches: vec![
            KagemushaSerializedSiblingBranchV7 {
                label: "recipient-child",
                statement: recipient_statement,
                public_inputs: recipient_public,
                opening: KagemushaSerializedNoteOpeningV7 {
                    rho: recipient_rho,
                    diversifier: recipient_diversifier,
                    leaf_index: recipient_paths.leaf_index,
                    path: recipient_paths.membership_path.clone(),
                },
            },
            KagemushaSerializedSiblingBranchV7 {
                label: "change-child",
                statement: change_statement,
                public_inputs: change_public,
                opening: KagemushaSerializedNoteOpeningV7 {
                    rho: change_rho,
                    diversifier: change_diversifier,
                    leaf_index: change_paths.leaf_index,
                    path: change_paths.membership_path.clone(),
                },
            },
        ],
        secure,
        output_membership,
        next_zero_path: append_paths.next_zero_path,
    })
}

struct KagemushaSerializedMergeMaterialV7 {
    statement: KagemushaRecursiveSpendPublicStatementV4,
    public_inputs: KagemushaPastaCyclePublicInputsV4,
    secure: super::confidential_v2::KagemushaStepSecureWitnessV3,
    output_membership: super::kagemusha_v2::KagemushaOutputMembershipWitnessV4,
}

#[allow(clippy::too_many_lines)]
fn kagemusha_serialized_merge_material_v7(
    material: &KagemushaSerializedReleaseMaterialV7,
    siblings: &KagemushaSerializedSiblingMaterialV7,
    parent_bundle_digests: [[u8; 32]; 2],
) -> Result<KagemushaSerializedMergeMaterialV7, String> {
    use super::{confidential_v2, kagemusha_v2};
    use iroha_data_model::offline::{
        KagemushaRecursiveSpendBranchV2, KagemushaRecursiveSpendInputBranchV2,
        KagemushaRecursiveSpendSplitIntentV4, KagemushaScaledAmountV2,
        KagemushaSpendableNoteDescriptorV2,
    };

    let [first, second] = siblings.branches.as_slice() else {
        return Err("Kagemusha V7 merge requires exactly two siblings".to_owned());
    };
    if parent_bundle_digests[0] >= parent_bundle_digests[1]
        || first.statement.final_root != second.statement.final_root
        || first.statement.next_zero_leaf_index != second.statement.next_zero_leaf_index
        || first.statement.topup_anchor_refs != second.statement.topup_anchor_refs
        || first.statement.artifact_binding != second.statement.artifact_binding
        || first.statement.current_note == second.statement.current_note
        || first.public_inputs.result_state == second.public_inputs.result_state
    {
        return Err("Kagemusha V7 sorted sibling merge preconditions failed".to_owned());
    }
    let root = first.statement.final_root;
    let next_zero_leaf_index = usize::try_from(first.statement.next_zero_leaf_index)
        .map_err(|_| "Kagemusha V7 merge frontier does not fit usize")?;
    if siblings.next_zero_path.root != root {
        return Err("Kagemusha V7 merge frontier does not match both sibling roots".to_owned());
    }
    for branch in [first, second] {
        let validated = confidential_v2::validate_confidential_membership_path_v3(
            branch.statement.current_note.note_commitment,
            branch.opening.leaf_index,
            &branch.opening.path,
        )?;
        if validated.root != root {
            return Err(
                "Kagemusha V7 merge input is not a real path under the common root".to_owned(),
            );
        }
    }
    if first.opening.leaf_index == second.opening.leaf_index
        || first
            .statement
            .current_note
            .amount
            .atomic_units
            .checked_add(second.statement.current_note.amount.atomic_units)
            != Some(KAGEMUSHA_SERIALIZED_FIXTURE_AMOUNT_V7)
    {
        return Err("Kagemusha V7 merge inputs are not two distinct conserved notes".to_owned());
    }
    let merge_rho = [0x7a; 32];
    let merge_spend_key = [0x7b; 32];
    let merge_diversifier = confidential_v2::derive_confidential_diversifier_v2(
        b"iroha:kagemusha:v7:release-proof:merge-output",
    );
    let merge_owner = confidential_v2::derive_confidential_owner_tag_v3_with_diversifier(
        &merge_spend_key,
        merge_diversifier,
    )?;
    let asset_tag =
        confidential_v2::derive_confidential_asset_tag_v3(&first.statement.asset.to_string())?;
    let network_tag =
        confidential_v2::derive_confidential_network_tag_v3(&first.statement.network_id)?;
    let merge_commitment = confidential_v2::derive_confidential_note_v3(
        asset_tag,
        KAGEMUSHA_SERIALIZED_FIXTURE_AMOUNT_V7,
        merge_rho,
        merge_owner,
    )?;
    let merge_nullifier = confidential_v2::derive_confidential_nullifier_v3(
        &merge_spend_key,
        merge_rho,
        asset_tag,
        network_tag,
    )?;
    let append_paths = confidential_v2::derive_confidential_sequential_append_paths_v3(
        next_zero_leaf_index,
        &siblings.next_zero_path,
        &[merge_commitment],
    )?;
    let [merge_paths] = append_paths.leaves.as_slice() else {
        return Err("Kagemusha V7 merge did not append exactly one output".to_owned());
    };
    let output_membership = kagemusha_v2::KagemushaOutputMembershipWitnessV4 {
        operation: kagemusha_v2::KagemushaOutputMembershipOperationV4::Split,
        initial_root: append_paths.initial_root,
        final_root: append_paths.final_root,
        recipient: Some(kagemusha_v2::KagemushaOutputMembershipLeafV4 {
            commitment: merge_commitment,
            leaf_index: u32::try_from(merge_paths.leaf_index)
                .map_err(|_| "Kagemusha V7 merge leaf index does not fit u32")?,
            update_path: kagemusha_calibration_membership_path_v4(merge_paths.update_path.clone()),
            membership_path: kagemusha_calibration_membership_path_v4(
                merge_paths.membership_path.clone(),
            ),
        }),
        change: None,
        dummy_leaf_index: u32::try_from(append_paths.next_zero_leaf_index)
            .map_err(|_| "Kagemusha V7 merge next frontier does not fit u32")?,
        dummy_path: kagemusha_calibration_membership_path_v4(append_paths.next_zero_path.clone()),
    };
    kagemusha_v2::KagemushaOutputMembershipCircuitV4::new(output_membership.clone())?;
    let amount_two = KagemushaScaledAmountV2::new(
        KAGEMUSHA_SERIALIZED_FIXTURE_AMOUNT_V7,
        first.statement.asset_scale,
    )
    .map_err(|error| error.to_string())?;
    let merge_note = KagemushaSpendableNoteDescriptorV2 {
        network_id: first.statement.network_id,
        asset: first.statement.asset.clone(),
        note_commitment: merge_commitment,
        spend_nullifier: merge_nullifier,
        amount: amount_two,
    };
    let inputs = siblings
        .branches
        .iter()
        .zip(parent_bundle_digests)
        .map(
            |(branch, bundle_digest)| KagemushaRecursiveSpendInputBranchV2 {
                bundle_digest,
                input_note: branch.statement.current_note.clone(),
                branch_claims: branch.statement.branch_claims.clone(),
                input_root: branch.statement.final_root,
                proof_step_count: branch.statement.proof_step_count,
                peer_hop_count: branch.statement.peer_hop_count,
            },
        )
        .collect();
    let split = KagemushaRecursiveSpendSplitIntentV4 {
        network_id: first.statement.network_id,
        asset: first.statement.asset.clone(),
        inputs,
        topup_anchor_refs: first.statement.topup_anchor_refs.clone(),
        asset_scale: first.statement.asset_scale,
        output_artifact_binding: first.statement.artifact_binding.clone(),
        transfer_amount: amount_two,
        recipient_output: merge_note,
        change_output: None,
        recipient_request_digest: [0x7c; 32],
        operation_id: [0x7d; 32],
    };
    if split.inputs.len() != 2
        || split.change_output.is_some()
        || split.transfer_amount.atomic_units != KAGEMUSHA_SERIALIZED_FIXTURE_AMOUNT_V7
        || split.recipient_output.amount.atomic_units != KAGEMUSHA_SERIALIZED_FIXTURE_AMOUNT_V7
    {
        return Err("Kagemusha V7 merge does not conserve two inputs into one output".to_owned());
    }
    split
        .validate_public_binding()
        .map_err(|error| error.to_string())?;
    let statement = kagemusha_v2::kagemusha_recursive_spend_append_statement_v4(
        &split,
        KagemushaRecursiveSpendBranchV2::Recipient,
        output_membership.final_root,
        output_membership.dummy_leaf_index,
    )?;
    let transfer_public = super::kagemusha_step_transition::KagemushaStepTransferPublicV4 {
        input_commitments: [
            first.statement.current_note.note_commitment,
            second.statement.current_note.note_commitment,
        ],
        input_nullifiers: [
            first.statement.current_note.spend_nullifier,
            second.statement.current_note.spend_nullifier,
        ],
        output_commitments: [merge_commitment, [0; 32]],
        root,
        asset_tag,
        network_tag,
    };
    let operation = KagemushaStepOperationVectorV4::from_append_v4(
        &split,
        &statement,
        &transfer_public,
        &output_membership,
    )?;
    let secure = confidential_v2::prepare_kagemusha_step_transfer_witness_v3_with_paths(
        &first.statement.network_id,
        &first.statement.asset.to_string(),
        &KAGEMUSHA_SERIALIZED_FIXTURE_BRANCH_SPEND_KEY_V7,
        &[first.opening.path.clone(), second.opening.path.clone()],
        &[
            confidential_v2::ConfidentialTransferInputV2 {
                amount: 1,
                rho: first.opening.rho,
                diversifier: first.opening.diversifier,
                leaf_index: first.opening.leaf_index,
            },
            confidential_v2::ConfidentialTransferInputV2 {
                amount: 1,
                rho: second.opening.rho,
                diversifier: second.opening.diversifier,
                leaf_index: second.opening.leaf_index,
            },
        ],
        &[confidential_v2::ConfidentialTransferOutputV2 {
            amount: KAGEMUSHA_SERIALIZED_FIXTURE_AMOUNT_V7,
            rho: merge_rho,
            owner_tag: merge_owner,
        }],
        root,
    )?;
    let mut public_inputs = kagemusha_v2::kagemusha_public_inputs_for_statement_v4(
        &statement,
        operation,
        material.manifest.sha256()?,
        material.manifest.step_eq_vk_sha256,
        material.manifest.step_ep_vk_sha256,
    )?;
    public_inputs.parent_count = 2;
    public_inputs.parent_states = [
        first.public_inputs.result_state.clone(),
        second.public_inputs.result_state.clone(),
    ];
    if public_inputs.parent_states[0] != first.public_inputs.result_state
        || public_inputs.parent_states[1] != second.public_inputs.result_state
        || split.inputs[0].bundle_digest != parent_bundle_digests[0]
        || split.inputs[1].bundle_digest != parent_bundle_digests[1]
        || transfer_public.input_commitments[0] != split.inputs[0].input_note.note_commitment
        || transfer_public.input_commitments[1] != split.inputs[1].input_note.note_commitment
    {
        return Err("Kagemusha V7 merge parent order/binding drifted".to_owned());
    }
    Ok(KagemushaSerializedMergeMaterialV7 {
        statement,
        public_inputs,
        secure,
        output_membership,
    })
}

struct KagemushaSerializedReleaseProofCaseV7<'a> {
    label: &'static str,
    proof_step_count: u32,
    public_inputs: KagemushaPastaCyclePublicInputsV4,
    secure: &'a super::confidential_v2::KagemushaStepSecureWitnessV3,
    output_membership: &'a super::kagemusha_v2::KagemushaOutputMembershipWitnessV4,
    parent_bundle_digests: Vec<[u8; 32]>,
    parent_indices: Vec<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct KagemushaSerializedReleaseCaseMeasurementV7 {
    label: &'static str,
    proof_step_count: u32,
    parent_count: u32,
    step_eq_proof_bytes: usize,
    step_ep_proof_bytes: usize,
    raw_proof_pair_bytes: usize,
    canonical_carrier_bytes: usize,
    canonical_carrier_sha256: [u8; 32],
    public_cells: usize,
    eq_coefficients: usize,
    ep_coefficients: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct KagemushaSerializedReleaseProofMeasurementV7 {
    null_carrier_bytes: usize,
    null_carrier_sha256: [u8; 32],
    manifest_sha256: [u8; 32],
    params_bytes: [usize; 2],
    verifying_key_bytes: [usize; 2],
    proving_key_bytes: [u64; 2],
    proving_key_sha256: [[u8; 32]; 2],
    conservative_peak_bytes: u64,
    active_memory_limit_bytes: u64,
    maximum_canonical_carrier_bytes: usize,
    canonical_carriers_fit_current_release_max: bool,
    cases: Vec<KagemushaSerializedReleaseCaseMeasurementV7>,
    mutation_rejections: usize,
    terminal_ipa_decisions: usize,
}

struct KagemushaSerializedReleaseMaterialV7 {
    manifest: super::kagemusha_serialized_audit_v7::KagemushaSerializedAuditManifestV7,
    circuit_params: KagemushaStepCircuitParamsV7,
    step_eq_params:
        halo2_proofs::poly::ipa::commitment::ParamsIPA<halo2_proofs::halo2curves::pasta::EqAffine>,
    step_ep_params:
        halo2_proofs::poly::ipa::commitment::ParamsIPA<halo2_proofs::halo2curves::pasta::EpAffine>,
    step_eq_verifying_key:
        halo2_proofs::plonk::VerifyingKey<halo2_proofs::halo2curves::pasta::EqAffine>,
    step_ep_verifying_key:
        halo2_proofs::plonk::VerifyingKey<halo2_proofs::halo2curves::pasta::EpAffine>,
    step_eq_protocol:
        snark_verifier::system::halo2::PlonkProtocol<halo2_proofs::halo2curves::pasta::EqAffine>,
    step_ep_protocol:
        snark_verifier::system::halo2::PlonkProtocol<halo2_proofs::halo2curves::pasta::EpAffine>,
    step_eq_structure_sha256: [u8; 32],
    step_ep_structure_sha256: [u8; 32],
    step_eq_break_points: Vec<Vec<u32>>,
    step_ep_break_points: Vec<Vec<u32>>,
    active_memory_limit_bytes: u64,
    reviewed_peak_bytes: u64,
    step_eq_proving_key_size_bytes: u64,
    step_ep_proving_key_size_bytes: u64,
    step_eq_proving_key_sha256: [u8; 32],
    step_ep_proving_key_sha256: [u8; 32],
    step_eq_null: KagemushaSerializedNullParentV7<halo2_proofs::halo2curves::pasta::EqAffine>,
    step_ep_null: KagemushaSerializedNullParentV7<halo2_proofs::halo2curves::pasta::EpAffine>,
    null_carrier: KagemushaSerializedSealedNullCarrierV7,
}

#[allow(clippy::too_many_lines)]
fn prepare_kagemusha_serialized_release_material_v7(
    mut manifest: super::kagemusha_serialized_audit_v7::KagemushaSerializedAuditManifestV7,
    memory_guard: &KagemushaGenerationMemoryGuardV4,
) -> Result<KagemushaSerializedReleaseMaterialV7, String> {
    use halo2_proofs::{
        SerdeFormat,
        halo2curves::pasta::{EpAffine, EqAffine},
        plonk::{keygen_pk_consuming_with, keygen_vk_consuming_with},
        poly::{
            commitment::{Params as _, ParamsProver as _},
            ipa::commitment::ParamsIPA,
        },
    };

    let mut base = KagemushaStepCircuitParamsV4::reviewed_first_release_generation_profile()?;
    base.max_parent_proof_bytes = manifest.step_eq_proof_bytes;
    let reviewed_peak_bytes = kagemusha_serialized_generation_peak_bound_from_base_v7(&base)?;
    if reviewed_peak_bytes != KAGEMUSHA_SERIALIZED_REVIEWED_PEAK_BYTES_V7 {
        return Err(format!(
            "Kagemusha V7 reviewed peak drifted from {KAGEMUSHA_SERIALIZED_REVIEWED_PEAK_BYTES_V7} to {reviewed_peak_bytes} bytes"
        ));
    }
    let active_memory_limit_bytes = memory_guard.effective_memory_limit_bytes();
    if active_memory_limit_bytes > KAGEMUSHA_GENERATION_REVIEWED_MAX_ESTIMATED_BYTES_V5 {
        return Err(format!(
            "Kagemusha V7 active guard ceiling {active_memory_limit_bytes} exceeds the explicit reviewed 56-GiB request"
        ));
    }
    if reviewed_peak_bytes > active_memory_limit_bytes {
        return Err(format!(
            "Kagemusha V7 reviewed peak {reviewed_peak_bytes} exceeds the active {active_memory_limit_bytes}-byte guarded ceiling"
        ));
    }
    let step_eq_params = ParamsIPA::<EqAffine>::new(manifest.k);
    let step_ep_params = ParamsIPA::<EpAffine>::new(manifest.k);
    for (role, encoded) in [
        ("Eq", {
            let mut encoded = Vec::new();
            step_eq_params
                .write(&mut encoded)
                .map_err(|error| format!("failed to encode Kagemusha V7 Eq ParamsIPA: {error}"))?;
            encoded
        }),
        ("Ep", {
            let mut encoded = Vec::new();
            step_ep_params
                .write(&mut encoded)
                .map_err(|error| format!("failed to encode Kagemusha V7 Ep ParamsIPA: {error}"))?;
            encoded
        }),
    ] {
        let size = u64::try_from(encoded.len())
            .map_err(|_| format!("Kagemusha V7 {role} ParamsIPA length does not fit u64"))?;
        if encoded.len() != KAGEMUSHA_SERIALIZED_PARAMS_BYTES_V7
            || size > KAGEMUSHA_COMPACT_PARAMS_IPA_MAX_BYTES_V5
        {
            return Err(format!(
                "Kagemusha V7 {role} ParamsIPA violates its exact/release cap"
            ));
        }
    }
    manifest.step_eq_params_sha256 = kagemusha_serialized_params_sha256_v7(&step_eq_params)?;
    manifest.step_ep_params_sha256 = kagemusha_serialized_params_sha256_v7(&step_ep_params)?;
    manifest.validate()?;
    let provisional_params = KagemushaStepCircuitParamsV7 {
        base: base.clone(),
        manifest: manifest.clone(),
    };
    provisional_params.validate()?;

    // These two ordinary zero-instance proofs only break the self-protocol
    // keygen cycle.  They are never returned as final null carriers.
    let step_eq_seed =
        kagemusha_serialized_eq_protocol_seed_v7(&step_eq_params, &provisional_params)?;
    let step_ep_seed =
        kagemusha_serialized_ep_protocol_seed_v7(&step_ep_params, &provisional_params)?;
    if step_eq_seed.identity_sha256 == step_ep_seed.identity_sha256 {
        return Err("Kagemusha V7 Eq/Ep seed protocol identities collide".to_owned());
    }
    manifest.step_eq_vk_sha256 = step_eq_seed.identity_sha256;
    manifest.step_ep_vk_sha256 = step_ep_seed.identity_sha256;
    manifest.validate()?;
    let seed_params = KagemushaStepCircuitParamsV7 {
        base: base.clone(),
        manifest: manifest.clone(),
    };
    let step_eq_seed_null =
        kagemusha_serialized_eq_null_from_seed_v7(&step_eq_params, &step_eq_seed, manifest.k)?;
    let step_ep_seed_null =
        kagemusha_serialized_ep_null_from_seed_v7(&step_ep_params, &step_ep_seed, manifest.k)?;
    let seed_calibration = kagemusha_rebind_calibration_manifest_v7(&manifest)?;
    let step_eq_seed_recursion = kagemusha_serialized_eq_recursion_from_null_v7(
        &step_eq_params,
        step_eq_seed.protocol.clone(),
        step_eq_seed.structure_sha256,
        &step_eq_seed_null,
    )?;
    let step_ep_seed_recursion = kagemusha_serialized_ep_recursion_from_null_v7(
        &step_ep_params,
        step_ep_seed.protocol.clone(),
        step_ep_seed.structure_sha256,
        &step_ep_seed_null,
    )?;
    let seed_witness = KagemushaStepWitnessV4 {
        public_inputs: &seed_calibration.public_inputs,
        proof_step_count: 1,
        secure: &seed_calibration.secure,
        output_membership: &seed_calibration.output_membership,
        step_eq_recursion: &step_eq_seed_recursion,
        step_ep_recursion: &step_ep_seed_recursion,
        step_eq_bootstrap: None,
        step_ep_bootstrap: None,
    };
    let seed_context = kagemusha_serialized_parent_digest_context_v7(&manifest)?;
    let seed_parent_digest =
        super::kagemusha_serialized_audit_v7::kagemusha_serialized_bytes_to_chunks_v7(
            super::kagemusha_serialized_audit_v7::kagemusha_serialized_null_parent_digest_v7(
                &seed_context,
            ),
        );
    let placeholder = kagemusha_serialized_placeholder_join_v7();
    let seed_public = KagemushaSerializedPublicInputsV7 {
        core: &seed_calibration.public_inputs,
        current_join: placeholder,
        parent_slots_digest: seed_parent_digest,
    };
    let (seed_eq_output, seed_eq_coefficients) =
        collect_kagemusha_serialized_native_audit_v7::<EqAffine>(
            &seed_public,
            1,
            &seed_params,
            &step_eq_seed_recursion,
        )?;
    let (seed_ep_output, seed_ep_coefficients) =
        collect_kagemusha_serialized_native_audit_v7::<EpAffine>(
            &seed_public,
            1,
            &seed_params,
            &step_ep_seed_recursion,
        )?;
    if u32::try_from(seed_eq_coefficients.len()).ok() != Some(manifest.eq_coefficient_count)
        || u32::try_from(seed_ep_coefficients.len()).ok() != Some(manifest.ep_coefficient_count)
    {
        return Err("Kagemusha V7 keygen prepass count differs from the manifest".to_owned());
    }
    let step_eq_keygen_circuit = build_kagemusha_step_eq_circuit_serialized_v7(
        &seed_witness,
        seed_params.clone(),
        &seed_public,
        &seed_ep_output,
        KagemushaSerializedPublicModeV7::NullParent,
        KagemushaCircuitBuilderStageV5::Keygen,
    )?;
    let (step_eq_verifying_key, step_eq_break_points) =
        keygen_vk_consuming_with(&step_eq_params, step_eq_keygen_circuit, |circuit| {
            kagemusha_break_points_to_wire_v5(circuit.builder.break_points(), &base)
        })
        .map_err(|error| {
            format_kagemusha_consuming_keygen_error_v5(
                error,
                "failed to generate Kagemusha V7 Eq verifying key",
            )
        })?;
    let step_ep_keygen_circuit = build_kagemusha_step_ep_circuit_serialized_v7(
        &seed_witness,
        seed_params.clone(),
        &seed_public,
        &seed_eq_output,
        KagemushaSerializedPublicModeV7::NullParent,
        KagemushaCircuitBuilderStageV5::Keygen,
    )?;
    let (step_ep_verifying_key, step_ep_break_points) =
        keygen_vk_consuming_with(&step_ep_params, step_ep_keygen_circuit, |circuit| {
            kagemusha_break_points_to_wire_v5(circuit.builder.break_points(), &base)
        })
        .map_err(|error| {
            format_kagemusha_consuming_keygen_error_v5(
                error,
                "failed to generate Kagemusha V7 Ep verifying key",
            )
        })?;
    let step_eq_protocol = snark_verifier::system::halo2::compile(
        &step_eq_params,
        &step_eq_verifying_key,
        kagemusha_ipa_compile_config_v4(KAGEMUSHA_SERIALIZED_PUBLIC_INSTANCE_CELLS_V7),
    );
    let step_ep_protocol = snark_verifier::system::halo2::compile(
        &step_ep_params,
        &step_ep_verifying_key,
        kagemusha_ipa_compile_config_v4(KAGEMUSHA_SERIALIZED_PUBLIC_INSTANCE_CELLS_V7),
    );
    let step_eq_structure_sha256 = kagemusha_require_protocol_structure_v1(
        &step_eq_seed.protocol,
        &step_eq_protocol,
        KagemushaPastaCycleParityV1::StepEq,
    )?;
    let step_ep_structure_sha256 = kagemusha_require_protocol_structure_v1(
        &step_ep_seed.protocol,
        &step_ep_protocol,
        KagemushaPastaCycleParityV1::StepEp,
    )?;
    manifest.step_eq_vk_sha256 = kagemusha_compiled_protocol_identity_sha256(
        &step_eq_protocol,
        KagemushaPastaCycleParityV1::StepEq,
    )?;
    manifest.step_ep_vk_sha256 = kagemusha_compiled_protocol_identity_sha256(
        &step_ep_protocol,
        KagemushaPastaCycleParityV1::StepEp,
    )?;
    if manifest.step_eq_vk_sha256 == manifest.step_ep_vk_sha256 {
        return Err("Kagemusha V7 final Eq/Ep protocol identities collide".to_owned());
    }
    manifest.validate()?;
    let circuit_params = KagemushaStepCircuitParamsV7 {
        base: base.clone(),
        manifest: manifest.clone(),
    };

    // Rebuild both keygen graphs after the final manifest/VK identities are
    // installed.  Exact VK bytes and breakpoints must converge, not merely the
    // value-free protocol structure.
    let final_calibration = kagemusha_rebind_calibration_manifest_v7(&manifest)?;
    let step_eq_final_seed_recursion = kagemusha_serialized_eq_recursion_from_null_v7(
        &step_eq_params,
        step_eq_protocol.clone(),
        step_eq_structure_sha256,
        &step_eq_seed_null,
    )?;
    let step_ep_final_seed_recursion = kagemusha_serialized_ep_recursion_from_null_v7(
        &step_ep_params,
        step_ep_protocol.clone(),
        step_ep_structure_sha256,
        &step_ep_seed_null,
    )?;
    let final_witness = KagemushaStepWitnessV4 {
        public_inputs: &final_calibration.public_inputs,
        proof_step_count: 1,
        secure: &final_calibration.secure,
        output_membership: &final_calibration.output_membership,
        step_eq_recursion: &step_eq_final_seed_recursion,
        step_ep_recursion: &step_ep_final_seed_recursion,
        step_eq_bootstrap: None,
        step_ep_bootstrap: None,
    };
    let final_context = kagemusha_serialized_parent_digest_context_v7(&manifest)?;
    let final_parent_digest =
        super::kagemusha_serialized_audit_v7::kagemusha_serialized_bytes_to_chunks_v7(
            super::kagemusha_serialized_audit_v7::kagemusha_serialized_null_parent_digest_v7(
                &final_context,
            ),
        );
    let final_public = KagemushaSerializedPublicInputsV7 {
        core: &final_calibration.public_inputs,
        current_join: placeholder,
        parent_slots_digest: final_parent_digest,
    };
    let (final_eq_output, final_eq_coefficients) =
        collect_kagemusha_serialized_native_audit_v7::<EqAffine>(
            &final_public,
            1,
            &circuit_params,
            &step_eq_final_seed_recursion,
        )?;
    let (final_ep_output, final_ep_coefficients) =
        collect_kagemusha_serialized_native_audit_v7::<EpAffine>(
            &final_public,
            1,
            &circuit_params,
            &step_ep_final_seed_recursion,
        )?;
    let step_eq_convergence_circuit = build_kagemusha_step_eq_circuit_serialized_v7(
        &final_witness,
        circuit_params.clone(),
        &final_public,
        &final_ep_output,
        KagemushaSerializedPublicModeV7::NullParent,
        KagemushaCircuitBuilderStageV5::Keygen,
    )?;
    let (step_eq_converged_vk, step_eq_converged_break_points) =
        keygen_vk_consuming_with(&step_eq_params, step_eq_convergence_circuit, |circuit| {
            kagemusha_break_points_to_wire_v5(circuit.builder.break_points(), &base)
        })
        .map_err(|error| {
            format_kagemusha_consuming_keygen_error_v5(
                error,
                "Kagemusha V7 Eq VK convergence failed",
            )
        })?;
    let step_ep_convergence_circuit = build_kagemusha_step_ep_circuit_serialized_v7(
        &final_witness,
        circuit_params.clone(),
        &final_public,
        &final_eq_output,
        KagemushaSerializedPublicModeV7::NullParent,
        KagemushaCircuitBuilderStageV5::Keygen,
    )?;
    let (step_ep_converged_vk, step_ep_converged_break_points) =
        keygen_vk_consuming_with(&step_ep_params, step_ep_convergence_circuit, |circuit| {
            kagemusha_break_points_to_wire_v5(circuit.builder.break_points(), &base)
        })
        .map_err(|error| {
            format_kagemusha_consuming_keygen_error_v5(
                error,
                "Kagemusha V7 Ep VK convergence failed",
            )
        })?;
    if step_eq_converged_vk.to_bytes(SerdeFormat::Processed)
        != step_eq_verifying_key.to_bytes(SerdeFormat::Processed)
        || step_ep_converged_vk.to_bytes(SerdeFormat::Processed)
            != step_ep_verifying_key.to_bytes(SerdeFormat::Processed)
        || step_eq_converged_break_points != step_eq_break_points
        || step_ep_converged_break_points != step_ep_break_points
    {
        return Err("Kagemusha V7 final VK/breakpoint convergence failed".to_owned());
    }
    validate_kagemusha_serialized_vk_geometry_v7(
        &step_eq_converged_vk,
        &manifest,
        KagemushaPastaCycleParityV1::StepEq,
    )?;
    validate_kagemusha_serialized_vk_geometry_v7(
        &step_ep_converged_vk,
        &manifest,
        KagemushaPastaCycleParityV1::StepEp,
    )?;

    let frozen_statement =
        kagemusha_serialized_frozen_statement_v7(&final_calibration.public_inputs, 1);
    let proof_bound = usize::try_from(manifest.step_eq_proof_bytes)
        .map_err(|_| "Kagemusha V7 proof bound does not fit usize".to_owned())?;
    let raw_proof_pair_bound =
        usize::try_from(KAGEMUSHA_RECURSIVE_SPEND_PROOF_PAIR_RELEASE_MAX_BYTES_V4)
            .map_err(|_| "Kagemusha V7 raw proof-pair bound does not fit usize".to_owned())?;
    let created_null = create_kagemusha_serialized_atomic_pair_once_v7(
        &step_eq_params,
        &step_ep_params,
        &step_eq_converged_vk,
        &step_ep_converged_vk,
        &manifest,
        frozen_statement,
        KagemushaSerializedPublicModeV7::NullParent,
        &final_eq_coefficients,
        &final_ep_coefficients,
        || {
            let circuit = build_kagemusha_step_eq_circuit_serialized_v7(
                &final_witness,
                circuit_params.clone(),
                &final_public,
                &final_ep_output,
                KagemushaSerializedPublicModeV7::NullParent,
                KagemushaCircuitBuilderStageV5::Keygen,
            )?;
            let (key, ()) = keygen_pk_consuming_with(
                &step_eq_params,
                step_eq_converged_vk.clone(),
                circuit,
                |circuit| {
                    ensure_kagemusha_keygen_break_points_v5(
                        &circuit.builder,
                        &base,
                        &step_eq_break_points,
                        "StepEq V7 null",
                    )
                },
            )
            .map_err(|error| {
                format_kagemusha_consuming_keygen_error_v5(
                    error,
                    "failed to generate Kagemusha V7 Eq null PK",
                )
            })?;
            prepare_kagemusha_serialized_eq_proving_key_v7(key, &step_eq_converged_vk, None)
        },
        || {
            let circuit = build_kagemusha_step_ep_circuit_serialized_v7(
                &final_witness,
                circuit_params.clone(),
                &final_public,
                &final_eq_output,
                KagemushaSerializedPublicModeV7::NullParent,
                KagemushaCircuitBuilderStageV5::Keygen,
            )?;
            let (key, ()) = keygen_pk_consuming_with(
                &step_ep_params,
                step_ep_converged_vk.clone(),
                circuit,
                |circuit| {
                    ensure_kagemusha_keygen_break_points_v5(
                        &circuit.builder,
                        &base,
                        &step_ep_break_points,
                        "StepEp V7 null",
                    )
                },
            )
            .map_err(|error| {
                format_kagemusha_consuming_keygen_error_v5(
                    error,
                    "failed to generate Kagemusha V7 Ep null PK",
                )
            })?;
            prepare_kagemusha_serialized_ep_proving_key_v7(key, &step_ep_converged_vk, None)
        },
        |join| {
            let public = KagemushaSerializedPublicInputsV7 {
                core: &final_calibration.public_inputs,
                current_join: join,
                parent_slots_digest: final_parent_digest,
            };
            let circuit = build_kagemusha_step_eq_circuit_serialized_v7(
                &final_witness,
                circuit_params.clone(),
                &public,
                &final_ep_output,
                KagemushaSerializedPublicModeV7::NullParent,
                KagemushaCircuitBuilderStageV5::Prover(&step_eq_break_points),
            )?;
            Ok((
                circuit,
                vec![vec![
                    Fp::ZERO;
                    KAGEMUSHA_SERIALIZED_PUBLIC_INSTANCE_CELLS_V7
                ]],
            ))
        },
        |join| {
            let public = KagemushaSerializedPublicInputsV7 {
                core: &final_calibration.public_inputs,
                current_join: join,
                parent_slots_digest: final_parent_digest,
            };
            let circuit = build_kagemusha_step_ep_circuit_serialized_v7(
                &final_witness,
                circuit_params.clone(),
                &public,
                &final_eq_output,
                KagemushaSerializedPublicModeV7::NullParent,
                KagemushaCircuitBuilderStageV5::Prover(&step_ep_break_points),
            )?;
            Ok((
                circuit,
                vec![vec![
                    Fq::ZERO;
                    KAGEMUSHA_SERIALIZED_PUBLIC_INSTANCE_CELLS_V7
                ]],
            ))
        },
        proof_bound,
        raw_proof_pair_bound,
    )?;
    if created_null.step_eq_proof.len() != KAGEMUSHA_SERIALIZED_STEP_PROOF_BYTES_V7
        || created_null.step_ep_proof.len() != KAGEMUSHA_SERIALIZED_STEP_PROOF_BYTES_V7
        || created_null.step_eq_proof.len() > KAGEMUSHA_SERIALIZED_STEP_PROOF_MAX_BYTES_V7
        || created_null.step_ep_proof.len() > KAGEMUSHA_SERIALIZED_STEP_PROOF_MAX_BYTES_V7
        || created_null.step_eq_proof.len() + created_null.step_ep_proof.len()
            != KAGEMUSHA_SERIALIZED_RAW_PROOF_PAIR_BYTES_V7
        || created_null.step_eq_proof.len() + created_null.step_ep_proof.len()
            > raw_proof_pair_bound
    {
        return Err("Kagemusha V7 final null proof sizes violate exact release caps".to_owned());
    }
    let step_eq_null_seed = KagemushaSerializedProtocolSeedV7 {
        protocol: step_eq_protocol.clone(),
        structure_sha256: step_eq_structure_sha256,
        identity_sha256: manifest.step_eq_vk_sha256,
        proof: created_null.step_eq_proof,
        current: created_null.verified.step_eq,
    };
    let step_ep_null_seed = KagemushaSerializedProtocolSeedV7 {
        protocol: step_ep_protocol.clone(),
        structure_sha256: step_ep_structure_sha256,
        identity_sha256: manifest.step_ep_vk_sha256,
        proof: created_null.step_ep_proof,
        current: created_null.verified.step_ep,
    };
    let step_eq_null =
        kagemusha_serialized_eq_null_from_seed_v7(&step_eq_params, &step_eq_null_seed, manifest.k)?;
    let step_ep_null =
        kagemusha_serialized_ep_null_from_seed_v7(&step_ep_params, &step_ep_null_seed, manifest.k)?;
    if created_null.step_eq_instances
        != vec![vec![
            Fp::ZERO;
            KAGEMUSHA_SERIALIZED_PUBLIC_INSTANCE_CELLS_V7
        ]]
        || created_null.step_ep_instances
            != vec![vec![
                Fq::ZERO;
                KAGEMUSHA_SERIALIZED_PUBLIC_INSTANCE_CELLS_V7
            ]]
    {
        return Err("Kagemusha V7 final null carrier is not the all-zero schema".to_owned());
    }
    if verify_kagemusha_serialized_atomic_pair_v7(
        &step_eq_params,
        &step_eq_converged_vk,
        &step_ep_params,
        &step_ep_converged_vk,
        &manifest,
        &step_eq_null.proof,
        &step_ep_null.proof,
        &created_null.step_eq_instances,
        &created_null.step_ep_instances,
        proof_bound,
        raw_proof_pair_bound,
    )
    .is_ok()
    {
        return Err("Kagemusha V7 NullParent-to-live substitution was accepted".to_owned());
    }
    let null_carrier =
        seal_kagemusha_serialized_null_carrier_v7(&manifest, &step_eq_null, &step_ep_null)?;

    Ok(KagemushaSerializedReleaseMaterialV7 {
        manifest,
        circuit_params,
        step_eq_params,
        step_ep_params,
        step_eq_verifying_key: step_eq_converged_vk,
        step_ep_verifying_key: step_ep_converged_vk,
        step_eq_protocol,
        step_ep_protocol,
        step_eq_structure_sha256,
        step_ep_structure_sha256,
        step_eq_break_points,
        step_ep_break_points,
        active_memory_limit_bytes,
        reviewed_peak_bytes,
        step_eq_proving_key_size_bytes: created_null.step_eq_proving_key_size_bytes,
        step_ep_proving_key_size_bytes: created_null.step_ep_proving_key_size_bytes,
        step_eq_proving_key_sha256: created_null.step_eq_proving_key_sha256,
        step_ep_proving_key_sha256: created_null.step_ep_proving_key_sha256,
        step_eq_null,
        step_ep_null,
        null_carrier,
    })
}

/// Canonical content-addressed V7 carrier.  Unlike the raw 186,368-byte proof
/// sum, this is the complete payload needed to authenticate and recurse from a
/// live node: both 70-cell columns, both proofs, both output lineages, and both
/// post-proof fold transcripts.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
struct KagemushaSerializedReleaseCarrierWireV7 {
    version: u16,
    manifest_sha256: [u8; 32],
    step_eq_instances: Vec<u128>,
    step_ep_instances: Vec<u128>,
    step_eq_proof_bytes: Vec<u8>,
    step_ep_proof_bytes: Vec<u8>,
    step_eq_lineage: KagemushaIpaAccumulatorWireV4,
    step_ep_lineage: KagemushaIpaAccumulatorWireV4,
    step_eq_post_proof_fold: KagemushaIpaAccumulationProofV4,
    step_ep_post_proof_fold: KagemushaIpaAccumulationProofV4,
}

impl KagemushaSerializedReleaseCarrierWireV7 {
    fn validate(&self, material: &KagemushaSerializedReleaseMaterialV7) -> Result<(), String> {
        if self.version != KAGEMUSHA_SERIALIZED_RELEASE_CARRIER_VERSION_V7
            || self.manifest_sha256 != material.manifest.sha256()?
            || self.step_eq_instances.len() != KAGEMUSHA_SERIALIZED_PUBLIC_INSTANCE_CELLS_V7
            || self.step_ep_instances.len() != KAGEMUSHA_SERIALIZED_PUBLIC_INSTANCE_CELLS_V7
            || self.step_eq_proof_bytes.len() != KAGEMUSHA_SERIALIZED_STEP_PROOF_BYTES_V7
            || self.step_ep_proof_bytes.len() != KAGEMUSHA_SERIALIZED_STEP_PROOF_BYTES_V7
            || self.step_eq_proof_bytes.len() > KAGEMUSHA_SERIALIZED_STEP_PROOF_MAX_BYTES_V7
            || self.step_ep_proof_bytes.len() > KAGEMUSHA_SERIALIZED_STEP_PROOF_MAX_BYTES_V7
            || self.step_eq_proof_bytes == self.step_ep_proof_bytes
            || self.step_eq_proof_bytes.len() + self.step_ep_proof_bytes.len()
                != KAGEMUSHA_SERIALIZED_RAW_PROOF_PAIR_BYTES_V7
        {
            return Err("Kagemusha V7 canonical carrier shape/identity drifted".to_owned());
        }
        let step_eq_instances = vec![
            self.step_eq_instances
                .iter()
                .copied()
                .map(Fp::from_u128)
                .collect::<Vec<_>>(),
        ];
        let step_ep_instances = vec![
            self.step_ep_instances
                .iter()
                .copied()
                .map(Fq::from_u128)
                .collect::<Vec<_>>(),
        ];
        let proof_bound = usize::try_from(material.manifest.step_eq_proof_bytes)
            .map_err(|_| "Kagemusha V7 carrier proof bound does not fit usize".to_owned())?;
        let raw_proof_pair_bound =
            usize::try_from(KAGEMUSHA_RECURSIVE_SPEND_PROOF_PAIR_RELEASE_MAX_BYTES_V4)
                .map_err(|_| "Kagemusha V7 raw proof-pair bound does not fit usize".to_owned())?;
        validate_kagemusha_serialized_atomic_envelope_v7(
            &material.manifest,
            &self.step_eq_proof_bytes,
            &self.step_ep_proof_bytes,
            &step_eq_instances,
            &step_ep_instances,
            proof_bound,
            raw_proof_pair_bound,
        )?;
        self.step_eq_lineage.to_eq(material.manifest.k)?;
        self.step_ep_lineage.to_ep(material.manifest.k)?;
        let parent_count = self.step_eq_instances[KAGEMUSHA_COMPACT_PARENT_COUNT_OFFSET_V5];
        let maximum_parent_count = u128::try_from(KAGEMUSHA_PASTA_PARENT_SLOTS_V1)
            .map_err(|_| "Kagemusha V7 parent-slot count does not fit u128".to_owned())?;
        if parent_count > maximum_parent_count
            || self.step_ep_instances[KAGEMUSHA_COMPACT_PARENT_COUNT_OFFSET_V5] != parent_count
        {
            return Err("Kagemusha V7 canonical carrier parent count drifted".to_owned());
        }
        let has_parent = parent_count != 0;
        self.step_eq_post_proof_fold
            .validate(material.manifest.k, has_parent)?;
        self.step_ep_post_proof_fold
            .validate(material.manifest.k, has_parent)?;
        Ok(())
    }
}

struct KagemushaSerializedReleaseCarrierV7 {
    proof_step_count: u32,
    public_inputs: KagemushaPastaCyclePublicInputsV4,
    pair: KagemushaSerializedCreatedPairV7,
    step_eq_lineage: KagemushaIpaAccumulatorWireV4,
    step_ep_lineage: KagemushaIpaAccumulatorWireV4,
    step_eq_post_proof_fold: KagemushaIpaAccumulationProofV4,
    step_ep_post_proof_fold: KagemushaIpaAccumulationProofV4,
    canonical_bytes: Vec<u8>,
    canonical_sha256: [u8; 32],
}

fn kagemusha_serialized_release_carrier_wire_v7(
    material: &KagemushaSerializedReleaseMaterialV7,
    carrier: &KagemushaSerializedReleaseCarrierV7,
) -> Result<KagemushaSerializedReleaseCarrierWireV7, String> {
    let step_eq_instances = kagemusha_serialized_instance_u128_v7(&carrier.pair.step_eq_instances)?;
    let step_ep_instances = kagemusha_serialized_instance_u128_v7(&carrier.pair.step_ep_instances)?;
    let join_cells: [u128; KAGEMUSHA_SERIALIZED_CURRENT_JOIN_CELLS_V7] = step_eq_instances
        [KAGEMUSHA_SERIALIZED_CURRENT_JOIN_OFFSET_V7
            ..KAGEMUSHA_SERIALIZED_CURRENT_JOIN_OFFSET_V7
                + KAGEMUSHA_SERIALIZED_CURRENT_JOIN_CELLS_V7]
        .try_into()
        .expect("V7 carrier join has ten cells");
    let current_join = super::kagemusha_serialized_audit_v7::KagemushaSerializedAuditPublicJoinV7 {
        step_eq_commitment: join_cells[..2]
            .try_into()
            .expect("V7 carrier Eq commitment has two chunks"),
        step_ep_commitment: join_cells[2..4]
            .try_into()
            .expect("V7 carrier Ep commitment has two chunks"),
        challenge: join_cells[4..6]
            .try_into()
            .expect("V7 carrier challenge has two chunks"),
        eq_evaluation: join_cells[6..8]
            .try_into()
            .expect("V7 carrier Eq evaluation has two chunks"),
        ep_evaluation: join_cells[8..10]
            .try_into()
            .expect("V7 carrier Ep evaluation has two chunks"),
    };
    let parent_slots_digest = step_eq_instances[KAGEMUSHA_SERIALIZED_PARENT_DIGEST_OFFSET_V7
        ..KAGEMUSHA_SERIALIZED_PARENT_DIGEST_OFFSET_V7
            + KAGEMUSHA_SERIALIZED_PARENT_DIGEST_CELLS_V7]
        .try_into()
        .expect("V7 carrier ancestry has two cells");
    let public = KagemushaSerializedPublicInputsV7 {
        core: &carrier.public_inputs,
        current_join,
        parent_slots_digest,
    };
    let expected_step_eq_instances = vec![public.instance_column::<Fp>(
        carrier.proof_step_count,
        &material.circuit_params.base,
        KagemushaPastaCycleParityV1::StepEq,
    )?];
    let expected_step_ep_instances = vec![public.instance_column::<Fq>(
        carrier.proof_step_count,
        &material.circuit_params.base,
        KagemushaPastaCycleParityV1::StepEp,
    )?];
    if carrier.pair.step_eq_instances != expected_step_eq_instances
        || carrier.pair.step_ep_instances != expected_step_ep_instances
    {
        return Err("Kagemusha V7 carrier host/public instance splice detected".to_owned());
    }
    if step_eq_instances[KAGEMUSHA_COMPACT_PROOF_STEP_COUNT_OFFSET_V5]
        != u128::from(carrier.proof_step_count)
        || step_ep_instances[KAGEMUSHA_COMPACT_PROOF_STEP_COUNT_OFFSET_V5]
            != u128::from(carrier.proof_step_count)
        || step_eq_instances[KAGEMUSHA_COMPACT_PARENT_COUNT_OFFSET_V5]
            != u128::from(carrier.public_inputs.parent_count)
        || step_ep_instances[KAGEMUSHA_COMPACT_PARENT_COUNT_OFFSET_V5]
            != u128::from(carrier.public_inputs.parent_count)
    {
        return Err("Kagemusha V7 carrier host/public topology splice detected".to_owned());
    }
    let expected_eq_parent = match &carrier.public_inputs.parent_eq_lineage_accumulator {
        Some(lineage) => lineage.instance_limbs(material.manifest.k)?,
        None => vec![
            0;
            KAGEMUSHA_SERIALIZED_CURRENT_JOIN_OFFSET_V7
                - KAGEMUSHA_SERIALIZED_COMMON_HEADER_CELLS_V7
        ],
    };
    let expected_ep_parent = match &carrier.public_inputs.parent_ep_lineage_accumulator {
        Some(lineage) => lineage.instance_limbs(material.manifest.k)?,
        None => vec![
            0;
            KAGEMUSHA_SERIALIZED_CURRENT_JOIN_OFFSET_V7
                - KAGEMUSHA_SERIALIZED_COMMON_HEADER_CELLS_V7
        ],
    };
    if step_eq_instances
        [KAGEMUSHA_SERIALIZED_COMMON_HEADER_CELLS_V7..KAGEMUSHA_SERIALIZED_CURRENT_JOIN_OFFSET_V7]
        != expected_eq_parent
        || step_ep_instances[KAGEMUSHA_SERIALIZED_COMMON_HEADER_CELLS_V7
            ..KAGEMUSHA_SERIALIZED_CURRENT_JOIN_OFFSET_V7]
            != expected_ep_parent
    {
        return Err("Kagemusha V7 carrier host/public lineage splice detected".to_owned());
    }
    let wire = KagemushaSerializedReleaseCarrierWireV7 {
        version: KAGEMUSHA_SERIALIZED_RELEASE_CARRIER_VERSION_V7,
        manifest_sha256: material.manifest.sha256()?,
        step_eq_instances: step_eq_instances.to_vec(),
        step_ep_instances: step_ep_instances.to_vec(),
        step_eq_proof_bytes: carrier.pair.step_eq_proof.clone(),
        step_ep_proof_bytes: carrier.pair.step_ep_proof.clone(),
        step_eq_lineage: carrier.step_eq_lineage.clone(),
        step_ep_lineage: carrier.step_ep_lineage.clone(),
        step_eq_post_proof_fold: carrier.step_eq_post_proof_fold.clone(),
        step_ep_post_proof_fold: carrier.step_ep_post_proof_fold.clone(),
    };
    wire.validate(material)?;
    Ok(wire)
}

fn seal_kagemusha_serialized_release_carrier_v7(
    material: &KagemushaSerializedReleaseMaterialV7,
    carrier: &mut KagemushaSerializedReleaseCarrierV7,
) -> Result<(), String> {
    let wire = kagemusha_serialized_release_carrier_wire_v7(material, carrier)?;
    let bytes = norito::encode_canonical(&wire)
        .map_err(|error| format!("failed to encode Kagemusha V7 canonical carrier: {error}"))?;
    let absolute_max = usize::try_from(KAGEMUSHA_RECURSIVE_SPEND_PROOF_PAIR_ABSOLUTE_MAX_BYTES_V4)
        .map_err(|_| "Kagemusha V7 absolute carrier bound does not fit usize".to_owned())?;
    if bytes.len() <= KAGEMUSHA_SERIALIZED_RAW_PROOF_PAIR_BYTES_V7 || bytes.len() > absolute_max {
        return Err(format!(
            "Kagemusha V7 canonical carrier is {} bytes and violates the existing {absolute_max}-byte absolute payload corridor; release is a hard NO",
            bytes.len()
        ));
    }
    let decoded: KagemushaSerializedReleaseCarrierWireV7 = norito::decode_canonical_with_limits(
        &bytes,
        kagemusha_runtime_norito_decode_limits_v4(bytes.len()),
    )
    .map_err(|error| {
        if matches!(&error, norito::Error::NonCanonicalEncoding) {
            "Kagemusha V7 carrier is not canonical Norito".to_owned()
        } else {
            format!("failed to decode Kagemusha V7 canonical carrier: {error}")
        }
    })?;
    if decoded != wire {
        return Err("Kagemusha V7 canonical carrier roundtrip drifted".to_owned());
    }
    decoded.validate(material)?;
    carrier.canonical_sha256 = Sha256::digest(&bytes).into();
    carrier.canonical_bytes = bytes;
    if carrier.canonical_sha256 == [0; 32] {
        return Err("Kagemusha V7 canonical carrier SHA-256 is zero".to_owned());
    }
    Ok(())
}

fn validate_kagemusha_serialized_release_carrier_seal_v7(
    material: &KagemushaSerializedReleaseMaterialV7,
    carrier: &KagemushaSerializedReleaseCarrierV7,
) -> Result<(), String> {
    let wire = kagemusha_serialized_release_carrier_wire_v7(material, carrier)?;
    let bytes = norito::encode_canonical(&wire)
        .map_err(|error| format!("failed to re-encode Kagemusha V7 carrier: {error}"))?;
    let sha256: [u8; 32] = Sha256::digest(&bytes).into();
    let absolute_max = usize::try_from(KAGEMUSHA_RECURSIVE_SPEND_PROOF_PAIR_ABSOLUTE_MAX_BYTES_V4)
        .map_err(|_| "Kagemusha V7 absolute carrier bound does not fit usize".to_owned())?;
    if bytes != carrier.canonical_bytes
        || bytes.len() <= KAGEMUSHA_SERIALIZED_RAW_PROOF_PAIR_BYTES_V7
        || bytes.len() > absolute_max
        || carrier.canonical_sha256 != sha256
        || carrier.canonical_sha256 == [0; 32]
    {
        return Err("Kagemusha V7 canonical carrier seal/size identity drifted".to_owned());
    }
    Ok(())
}

fn kagemusha_serialized_eq_parent_from_carrier_v7(
    material: &KagemushaSerializedReleaseMaterialV7,
    carrier: &KagemushaSerializedReleaseCarrierV7,
) -> Result<KagemushaStepParentProofV4<halo2_proofs::halo2curves::pasta::EqAffine>, String> {
    let (carried_lineage, external_accumulation_proof) = if carrier.public_inputs.parent_count == 0
    {
        (
            material.step_eq_null.current.clone(),
            material.step_eq_null.post_proof_fold.clone(),
        )
    } else {
        (
            carrier
                .public_inputs
                .parent_eq_lineage_accumulator
                .as_ref()
                .ok_or_else(|| "Kagemusha V7 Eq parent omitted its carried lineage".to_owned())?
                .to_eq(material.manifest.k)?,
            carrier.step_eq_post_proof_fold.clone(),
        )
    };
    Ok(KagemushaStepParentProofV4 {
        instances: carrier.pair.step_eq_instances.clone(),
        proof_bytes: carrier.pair.step_eq_proof.clone(),
        carried_lineage,
        external_accumulation_proof,
    })
}

fn kagemusha_serialized_ep_parent_from_carrier_v7(
    material: &KagemushaSerializedReleaseMaterialV7,
    carrier: &KagemushaSerializedReleaseCarrierV7,
) -> Result<KagemushaStepParentProofV4<halo2_proofs::halo2curves::pasta::EpAffine>, String> {
    let (carried_lineage, external_accumulation_proof) = if carrier.public_inputs.parent_count == 0
    {
        (
            material.step_ep_null.current.clone(),
            material.step_ep_null.post_proof_fold.clone(),
        )
    } else {
        (
            carrier
                .public_inputs
                .parent_ep_lineage_accumulator
                .as_ref()
                .ok_or_else(|| "Kagemusha V7 Ep parent omitted its carried lineage".to_owned())?
                .to_ep(material.manifest.k)?,
            carrier.step_ep_post_proof_fold.clone(),
        )
    };
    Ok(KagemushaStepParentProofV4 {
        instances: carrier.pair.step_ep_instances.clone(),
        proof_bytes: carrier.pair.step_ep_proof.clone(),
        carried_lineage,
        external_accumulation_proof,
    })
}

fn kagemusha_serialized_parent_slot_from_carrier_v7(
    carrier: &KagemushaSerializedReleaseCarrierV7,
) -> Result<super::kagemusha_serialized_audit_v7::KagemushaSerializedParentSlotV7, String> {
    let eq = kagemusha_serialized_instance_u128_v7(&carrier.pair.step_eq_instances)?;
    let ep = kagemusha_serialized_instance_u128_v7(&carrier.pair.step_ep_instances)?;
    if eq[..KAGEMUSHA_SERIALIZED_COMMON_HEADER_CELLS_V7]
        != ep[..KAGEMUSHA_SERIALIZED_COMMON_HEADER_CELLS_V7]
        || eq[KAGEMUSHA_SERIALIZED_CURRENT_JOIN_OFFSET_V7..]
            != ep[KAGEMUSHA_SERIALIZED_CURRENT_JOIN_OFFSET_V7..]
        || eq[KAGEMUSHA_SERIALIZED_LIVE_SELECTOR_OFFSET_V7] != 1
    {
        return Err("Kagemusha V7 real parent pair is not live/cross-matched".to_owned());
    }
    Ok(
        super::kagemusha_serialized_audit_v7::KagemushaSerializedParentSlotV7 {
            present: true,
            profile: eq[KAGEMUSHA_COMPACT_PROFILE_OFFSET_V5],
            proof_step_count: eq[KAGEMUSHA_COMPACT_PROOF_STEP_COUNT_OFFSET_V5],
            parent_count: u8::try_from(eq[KAGEMUSHA_COMPACT_PARENT_COUNT_OFFSET_V5])
                .map_err(|_| "Kagemusha V7 parent count does not fit u8".to_owned())?,
            parent_live: true,
            frozen_core_statement: eq[KAGEMUSHA_COMPACT_STATEMENT_DIGEST_OFFSET_V5
                ..KAGEMUSHA_COMPACT_STATEMENT_DIGEST_OFFSET_V5 + 2]
                .try_into()
                .expect("V7 statement has two chunks"),
            current_join: eq[KAGEMUSHA_SERIALIZED_CURRENT_JOIN_OFFSET_V7
                ..KAGEMUSHA_SERIALIZED_CURRENT_JOIN_OFFSET_V7
                    + KAGEMUSHA_SERIALIZED_CURRENT_JOIN_CELLS_V7]
                .try_into()
                .expect("V7 join has ten cells"),
            parent_slots_digest: eq[KAGEMUSHA_SERIALIZED_PARENT_DIGEST_OFFSET_V7
                ..KAGEMUSHA_SERIALIZED_PARENT_DIGEST_OFFSET_V7
                    + KAGEMUSHA_SERIALIZED_PARENT_DIGEST_CELLS_V7]
                .try_into()
                .expect("V7 ancestry has two cells"),
        },
    )
}

fn kagemusha_serialized_carrier_bundle_digest_v7(
    material: &KagemushaSerializedReleaseMaterialV7,
    carrier: &KagemushaSerializedReleaseCarrierV7,
) -> Result<[u8; 32], String> {
    validate_kagemusha_serialized_release_carrier_seal_v7(material, carrier)?;
    // Offline inputs define `bundle_digest` as the complete previous recursive
    // bundle identity.  This sealed canonical SHA is host-checked before proof
    // construction, while the circuit independently authenticates the exact
    // parent proof instances through `parent_slots_digest`.
    Ok(carrier.canonical_sha256)
}

#[allow(clippy::too_many_lines)]
fn prepare_kagemusha_serialized_case_recursions_v7(
    material: &KagemushaSerializedReleaseMaterialV7,
    public_inputs: &mut KagemushaPastaCyclePublicInputsV4,
    parents: &[&KagemushaSerializedReleaseCarrierV7],
) -> Result<
    (
        KagemushaStepParityRecursionV4<halo2_proofs::halo2curves::pasta::EqAffine>,
        KagemushaStepParityRecursionV4<halo2_proofs::halo2curves::pasta::EpAffine>,
        [u128; KAGEMUSHA_SERIALIZED_PARENT_DIGEST_CELLS_V7],
    ),
    String,
> {
    if parents.len() > KAGEMUSHA_PASTA_PARENT_SLOTS_V1
        || usize::try_from(public_inputs.parent_count).ok() != Some(parents.len())
    {
        return Err("Kagemusha V7 case parent count is invalid".to_owned());
    }
    public_inputs.parent_eq_deferred_sha256 = [[0; 8]; KAGEMUSHA_PASTA_PARENT_SLOTS_V1];
    public_inputs.parent_ep_deferred_sha256 = [[0; 8]; KAGEMUSHA_PASTA_PARENT_SLOTS_V1];
    let (parent_eq_lineage, eq_branch_merge_fold) = match parents {
        [] => (None, material.step_eq_null.branch_merge_fold.clone()),
        [parent] => (
            Some(parent.step_eq_lineage.clone()),
            material.step_eq_null.branch_merge_fold.clone(),
        ),
        [first, second] => {
            let (fold, accumulated) = super::kagemusha_accumulation::fold_eq_accumulators_v4(
                &material.step_eq_params,
                material.manifest.k,
                first.step_eq_lineage.to_eq(material.manifest.k)?,
                Some(second.step_eq_lineage.to_eq(material.manifest.k)?),
            )?;
            (
                Some(KagemushaIpaAccumulatorWireV4::from_eq(
                    &accumulated,
                    material.manifest.k,
                )?),
                fold,
            )
        }
        _ => unreachable!("V7 parent count was bounded above"),
    };
    let (parent_ep_lineage, ep_branch_merge_fold) = match parents {
        [] => (None, material.step_ep_null.branch_merge_fold.clone()),
        [parent] => (
            Some(parent.step_ep_lineage.clone()),
            material.step_ep_null.branch_merge_fold.clone(),
        ),
        [first, second] => {
            let (fold, accumulated) = super::kagemusha_accumulation::fold_ep_accumulators_v4(
                &material.step_ep_params,
                material.manifest.k,
                first.step_ep_lineage.to_ep(material.manifest.k)?,
                Some(second.step_ep_lineage.to_ep(material.manifest.k)?),
            )?;
            (
                Some(KagemushaIpaAccumulatorWireV4::from_ep(
                    &accumulated,
                    material.manifest.k,
                )?),
                fold,
            )
        }
        _ => unreachable!("V7 parent count was bounded above"),
    };
    public_inputs.parent_eq_lineage_accumulator = parent_eq_lineage;
    public_inputs.parent_ep_lineage_accumulator = parent_ep_lineage;

    let mut eq_parents = Vec::with_capacity(KAGEMUSHA_PASTA_PARENT_SLOTS_V1);
    let mut ep_parents = Vec::with_capacity(KAGEMUSHA_PASTA_PARENT_SLOTS_V1);
    let mut digest_slots =
        [super::kagemusha_serialized_audit_v7::KagemushaSerializedParentSlotV7::default();
            KAGEMUSHA_PASTA_PARENT_SLOTS_V1];
    for slot in 0..KAGEMUSHA_PASTA_PARENT_SLOTS_V1 {
        if let Some(parent) = parents.get(slot) {
            eq_parents.push(kagemusha_serialized_eq_parent_from_carrier_v7(
                material, parent,
            )?);
            ep_parents.push(kagemusha_serialized_ep_parent_from_carrier_v7(
                material, parent,
            )?);
            digest_slots[slot] = kagemusha_serialized_parent_slot_from_carrier_v7(parent)?;
        } else {
            eq_parents.push(material.step_eq_null.parent());
            ep_parents.push(material.step_ep_null.parent());
        }
    }
    let step_eq = KagemushaStepParityRecursionV4 {
        succinct_vk: kagemusha_eq_succinct_vk_v4(&material.step_eq_params)?,
        compiled_parent_protocol: material.step_eq_protocol.clone(),
        fixed_structure_sha256: material.step_eq_structure_sha256,
        parents: eq_parents.try_into().map_err(|parents: Vec<_>| {
            format!(
                "Kagemusha V7 Eq recursion has {} parent slots",
                parents.len()
            )
        })?,
        branch_merge_fold: eq_branch_merge_fold,
    };
    let step_ep = KagemushaStepParityRecursionV4 {
        succinct_vk: kagemusha_ep_succinct_vk_v4(&material.step_ep_params)?,
        compiled_parent_protocol: material.step_ep_protocol.clone(),
        fixed_structure_sha256: material.step_ep_structure_sha256,
        parents: ep_parents.try_into().map_err(|parents: Vec<_>| {
            format!(
                "Kagemusha V7 Ep recursion has {} parent slots",
                parents.len()
            )
        })?,
        branch_merge_fold: ep_branch_merge_fold,
    };
    let parent_digest =
        super::kagemusha_serialized_audit_v7::kagemusha_serialized_parent_slots_digest_v7(
            &kagemusha_serialized_parent_digest_context_v7(&material.manifest)?,
            digest_slots,
        )?;
    Ok((
        step_eq,
        step_ep,
        super::kagemusha_serialized_audit_v7::kagemusha_serialized_bytes_to_chunks_v7(
            parent_digest,
        ),
    ))
}

#[allow(clippy::too_many_lines)]
fn assert_kagemusha_serialized_live_mutations_fail_v7(
    material: &KagemushaSerializedReleaseMaterialV7,
    pair: &KagemushaSerializedCreatedPairV7,
) -> Result<usize, String> {
    let proof_bound = usize::try_from(material.manifest.step_eq_proof_bytes)
        .map_err(|_| "Kagemusha V7 proof bound does not fit usize".to_owned())?;
    let raw_proof_pair_bound =
        usize::try_from(KAGEMUSHA_RECURSIVE_SPEND_PROOF_PAIR_RELEASE_MAX_BYTES_V4)
            .map_err(|_| "Kagemusha V7 raw proof-pair bound does not fit usize".to_owned())?;
    validate_kagemusha_serialized_atomic_envelope_v7(
        &material.manifest,
        &pair.step_eq_proof,
        &pair.step_ep_proof,
        &pair.step_eq_instances,
        &pair.step_ep_instances,
        proof_bound,
        raw_proof_pair_bound,
    )?;
    let rejected = std::cell::Cell::new(0_usize);
    let reject_envelope = |eq_instances: &[Vec<Fp>], ep_instances: &[Vec<Fq>]| {
        if validate_kagemusha_serialized_atomic_envelope_v7(
            &material.manifest,
            &pair.step_eq_proof,
            &pair.step_ep_proof,
            eq_instances,
            ep_instances,
            proof_bound,
            raw_proof_pair_bound,
        )
        .is_ok()
        {
            return Err("Kagemusha V7 adversarial envelope mutation was accepted".to_owned());
        }
        rejected.set(
            rejected
                .get()
                .checked_add(1)
                .ok_or_else(|| "Kagemusha V7 mutation count overflowed".to_owned())?,
        );
        Ok(())
    };
    let mut bad_eq = pair.step_eq_instances.clone();
    bad_eq[0][KAGEMUSHA_COMPACT_PROFILE_OFFSET_V5] += Fp::ONE;
    reject_envelope(&bad_eq, &pair.step_ep_instances)?;
    let mut bad_ep = pair.step_ep_instances.clone();
    bad_ep[0][KAGEMUSHA_SERIALIZED_LIVE_SELECTOR_OFFSET_V7] = Fq::ZERO;
    reject_envelope(&pair.step_eq_instances, &bad_ep)?;
    let mut bad_ep = pair.step_ep_instances.clone();
    bad_ep[0][KAGEMUSHA_SERIALIZED_PARENT_DIGEST_OFFSET_V7] += Fq::ONE;
    reject_envelope(&pair.step_eq_instances, &bad_ep)?;
    let mut bad_eq = pair.step_eq_instances.clone();
    bad_eq[0][KAGEMUSHA_COMPACT_STATEMENT_DIGEST_OFFSET_V5] += Fp::ONE;
    reject_envelope(&bad_eq, &pair.step_ep_instances)?;

    let mut wrong_manifest = material.manifest.clone();
    wrong_manifest.step_eq_vk_sha256[0] ^= 1;
    if validate_kagemusha_serialized_atomic_envelope_v7(
        &wrong_manifest,
        &pair.step_eq_proof,
        &pair.step_ep_proof,
        &pair.step_eq_instances,
        &pair.step_ep_instances,
        proof_bound,
        raw_proof_pair_bound,
    )
    .is_ok()
    {
        return Err("Kagemusha V7 wrong-manifest mutation was accepted".to_owned());
    }
    rejected.set(rejected.get() + 1);

    let mut bad_eq_proof = pair.step_eq_proof.clone();
    let middle = bad_eq_proof.len() / 2;
    bad_eq_proof[middle] ^= 1;
    if verify_kagemusha_serialized_atomic_pair_v7(
        &material.step_eq_params,
        &material.step_eq_verifying_key,
        &material.step_ep_params,
        &material.step_ep_verifying_key,
        &material.manifest,
        &bad_eq_proof,
        &pair.step_ep_proof,
        &pair.step_eq_instances,
        &pair.step_ep_instances,
        proof_bound,
        raw_proof_pair_bound,
    )
    .is_ok()
    {
        return Err("Kagemusha V7 mutated Eq proof was accepted".to_owned());
    }
    rejected.set(rejected.get() + 1);
    let mut bad_ep_proof = pair.step_ep_proof.clone();
    let middle = bad_ep_proof.len() / 2;
    bad_ep_proof[middle] ^= 1;
    if verify_kagemusha_serialized_atomic_pair_v7(
        &material.step_eq_params,
        &material.step_eq_verifying_key,
        &material.step_ep_params,
        &material.step_ep_verifying_key,
        &material.manifest,
        &pair.step_eq_proof,
        &bad_ep_proof,
        &pair.step_eq_instances,
        &pair.step_ep_instances,
        proof_bound,
        raw_proof_pair_bound,
    )
    .is_ok()
    {
        return Err("Kagemusha V7 mutated Ep proof was accepted".to_owned());
    }
    rejected.set(rejected.get() + 1);

    let null_eq_instances = vec![vec![
        Fp::ZERO;
        KAGEMUSHA_SERIALIZED_PUBLIC_INSTANCE_CELLS_V7
    ]];
    let null_ep_instances = vec![vec![
        Fq::ZERO;
        KAGEMUSHA_SERIALIZED_PUBLIC_INSTANCE_CELLS_V7
    ]];
    reject_envelope(&null_eq_instances, &null_ep_instances)?;
    if verify_kagemusha_serialized_null_parent_pair_v7(
        &material.step_eq_params,
        &material.step_eq_verifying_key,
        &material.step_ep_params,
        &material.step_ep_verifying_key,
        &material.manifest,
        &pair.step_eq_proof,
        &pair.step_ep_proof,
        &pair.step_eq_instances,
        &pair.step_ep_instances,
        proof_bound,
        raw_proof_pair_bound,
    )
    .is_ok()
    {
        return Err("Kagemusha V7 live-to-null substitution was accepted".to_owned());
    }
    rejected.set(rejected.get() + 1);
    if validate_kagemusha_serialized_atomic_envelope_v7(
        &material.manifest,
        &pair.step_eq_proof,
        &pair.step_ep_proof,
        &pair.step_eq_instances,
        &pair.step_ep_instances,
        proof_bound,
        pair.step_eq_proof.len() + pair.step_ep_proof.len() - 1,
    )
    .is_ok()
    {
        return Err("Kagemusha V7 undersized pair bound was accepted".to_owned());
    }
    rejected.set(rejected.get() + 1);
    Ok(rejected.get())
}

struct KagemushaSerializedProvedCaseV7 {
    carrier: KagemushaSerializedReleaseCarrierV7,
    measurement: KagemushaSerializedReleaseCaseMeasurementV7,
    mutation_rejections: usize,
    terminal_ipa_decisions: usize,
}

#[allow(clippy::too_many_lines)]
fn prove_kagemusha_serialized_release_case_v7(
    material: &KagemushaSerializedReleaseMaterialV7,
    expected_manifest_sha256: [u8; 32],
    case: &mut KagemushaSerializedReleaseProofCaseV7<'_>,
    carriers: &[KagemushaSerializedReleaseCarrierV7],
) -> Result<KagemushaSerializedProvedCaseV7, String> {
    use halo2_proofs::{
        halo2curves::pasta::{EpAffine, EqAffine},
        plonk::keygen_pk_consuming_with,
    };

    if material.manifest.sha256()? != expected_manifest_sha256 {
        return Err("Kagemusha V7 driver manifest differs from the trusted expectation".to_owned());
    }
    let proof_bound = usize::try_from(material.manifest.step_eq_proof_bytes)
        .map_err(|_| "Kagemusha V7 proof bound does not fit usize".to_owned())?;
    let raw_proof_pair_bound =
        usize::try_from(KAGEMUSHA_RECURSIVE_SPEND_PROOF_PAIR_RELEASE_MAX_BYTES_V4)
            .map_err(|_| "Kagemusha V7 raw proof-pair bound does not fit usize".to_owned())?;
    if proof_bound != KAGEMUSHA_SERIALIZED_STEP_PROOF_BYTES_V7
        || proof_bound > KAGEMUSHA_SERIALIZED_STEP_PROOF_MAX_BYTES_V7
        || KAGEMUSHA_SERIALIZED_RAW_PROOF_PAIR_BYTES_V7 > raw_proof_pair_bound
    {
        return Err("Kagemusha V7 configured proof limits violate exact release caps".to_owned());
    }
    let manifest_limbs = kagemusha_calibration_exact_limbs_v4(expected_manifest_sha256);
    let eq_protocol_words = kagemusha_sha256_public_words(material.manifest.step_eq_vk_sha256);
    let ep_protocol_words = kagemusha_sha256_public_words(material.manifest.step_ep_vk_sha256);
    let placeholder = kagemusha_serialized_placeholder_join_v7();
    if case.public_inputs.manifest_sha256 != manifest_limbs
        || case.public_inputs.step_eq_compiled_protocol_sha256 != eq_protocol_words
        || case.public_inputs.step_ep_compiled_protocol_sha256 != ep_protocol_words
        || case.public_inputs.live_selector != KAGEMUSHA_PASTA_PUBLIC_LIVE_SELECTOR_V4
        || usize::try_from(case.public_inputs.parent_count).ok() != Some(case.parent_indices.len())
        || case.parent_bundle_digests.len() != case.parent_indices.len()
        || (case.proof_step_count == 1) != case.parent_indices.is_empty()
    {
        return Err(format!(
            "Kagemusha V7 case {} has the wrong identity/topology",
            case.label
        ));
    }
    let parents = case
        .parent_indices
        .iter()
        .map(|index| {
            carriers.get(*index).ok_or_else(|| {
                format!(
                    "Kagemusha V7 case {} references a future parent",
                    case.label
                )
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    for (slot, parent) in parents.iter().enumerate() {
        if case.public_inputs.parent_states[slot] != parent.public_inputs.result_state
            || case.parent_bundle_digests[slot]
                != kagemusha_serialized_carrier_bundle_digest_v7(material, parent)?
        {
            return Err(format!(
                "Kagemusha V7 case {} parent proof/state identity {slot} is stale",
                case.label
            ));
        }
        verify_kagemusha_serialized_atomic_pair_v7(
            &material.step_eq_params,
            &material.step_eq_verifying_key,
            &material.step_ep_params,
            &material.step_ep_verifying_key,
            &material.manifest,
            &parent.pair.step_eq_proof,
            &parent.pair.step_ep_proof,
            &parent.pair.step_eq_instances,
            &parent.pair.step_ep_instances,
            proof_bound,
            raw_proof_pair_bound,
        )?;
    }
    let parent_decisions = parents
        .len()
        .checked_mul(2)
        .ok_or_else(|| "Kagemusha V7 parent decision count overflowed".to_owned())?;
    let (step_eq_recursion, step_ep_recursion, parent_slots_digest) =
        prepare_kagemusha_serialized_case_recursions_v7(
            material,
            &mut case.public_inputs,
            &parents,
        )?;
    let witness = KagemushaStepWitnessV4 {
        public_inputs: &case.public_inputs,
        proof_step_count: case.proof_step_count,
        secure: case.secure,
        output_membership: case.output_membership,
        step_eq_recursion: &step_eq_recursion,
        step_ep_recursion: &step_ep_recursion,
        step_eq_bootstrap: None,
        step_ep_bootstrap: None,
    };
    let public = KagemushaSerializedPublicInputsV7 {
        core: &case.public_inputs,
        current_join: placeholder,
        parent_slots_digest,
    };
    let (eq_output, eq_coefficients) = collect_kagemusha_serialized_native_audit_v7::<EqAffine>(
        &public,
        case.proof_step_count,
        &material.circuit_params,
        &step_eq_recursion,
    )?;
    let (ep_output, ep_coefficients) = collect_kagemusha_serialized_native_audit_v7::<EpAffine>(
        &public,
        case.proof_step_count,
        &material.circuit_params,
        &step_ep_recursion,
    )?;
    if u32::try_from(eq_coefficients.len()).ok() != Some(material.manifest.eq_coefficient_count)
        || u32::try_from(ep_coefficients.len()).ok() != Some(material.manifest.ep_coefficient_count)
    {
        return Err(format!(
            "Kagemusha V7 case {} coefficient recapture drifted",
            case.label
        ));
    }
    let frozen_statement =
        kagemusha_serialized_frozen_statement_v7(&case.public_inputs, case.proof_step_count);
    let pair = create_kagemusha_serialized_atomic_pair_once_v7(
        &material.step_eq_params,
        &material.step_ep_params,
        &material.step_eq_verifying_key,
        &material.step_ep_verifying_key,
        &material.manifest,
        frozen_statement,
        KagemushaSerializedPublicModeV7::Live,
        &eq_coefficients,
        &ep_coefficients,
        || {
            let circuit = build_kagemusha_step_eq_circuit_serialized_v7(
                &witness,
                material.circuit_params.clone(),
                &public,
                &ep_output,
                KagemushaSerializedPublicModeV7::Live,
                KagemushaCircuitBuilderStageV5::Keygen,
            )?;
            let (key, ()) = keygen_pk_consuming_with(
                &material.step_eq_params,
                material.step_eq_verifying_key.clone(),
                circuit,
                |circuit| {
                    ensure_kagemusha_keygen_break_points_v5(
                        &circuit.builder,
                        &material.circuit_params.base,
                        &material.step_eq_break_points,
                        "StepEq V7 live",
                    )
                },
            )
            .map_err(|error| {
                format_kagemusha_consuming_keygen_error_v5(
                    error,
                    "failed to regenerate Kagemusha V7 Eq live PK",
                )
            })?;
            prepare_kagemusha_serialized_eq_proving_key_v7(
                key,
                &material.step_eq_verifying_key,
                Some(material.step_eq_proving_key_sha256),
            )
        },
        || {
            let circuit = build_kagemusha_step_ep_circuit_serialized_v7(
                &witness,
                material.circuit_params.clone(),
                &public,
                &eq_output,
                KagemushaSerializedPublicModeV7::Live,
                KagemushaCircuitBuilderStageV5::Keygen,
            )?;
            let (key, ()) = keygen_pk_consuming_with(
                &material.step_ep_params,
                material.step_ep_verifying_key.clone(),
                circuit,
                |circuit| {
                    ensure_kagemusha_keygen_break_points_v5(
                        &circuit.builder,
                        &material.circuit_params.base,
                        &material.step_ep_break_points,
                        "StepEp V7 live",
                    )
                },
            )
            .map_err(|error| {
                format_kagemusha_consuming_keygen_error_v5(
                    error,
                    "failed to regenerate Kagemusha V7 Ep live PK",
                )
            })?;
            prepare_kagemusha_serialized_ep_proving_key_v7(
                key,
                &material.step_ep_verifying_key,
                Some(material.step_ep_proving_key_sha256),
            )
        },
        |join| {
            let public = KagemushaSerializedPublicInputsV7 {
                core: &case.public_inputs,
                current_join: join,
                parent_slots_digest,
            };
            let circuit = build_kagemusha_step_eq_circuit_serialized_v7(
                &witness,
                material.circuit_params.clone(),
                &public,
                &ep_output,
                KagemushaSerializedPublicModeV7::Live,
                KagemushaCircuitBuilderStageV5::Prover(&material.step_eq_break_points),
            )?;
            let instances = vec![public.instance_column::<Fp>(
                case.proof_step_count,
                &material.circuit_params.base,
                KagemushaPastaCycleParityV1::StepEq,
            )?];
            Ok((circuit, instances))
        },
        |join| {
            let public = KagemushaSerializedPublicInputsV7 {
                core: &case.public_inputs,
                current_join: join,
                parent_slots_digest,
            };
            let circuit = build_kagemusha_step_ep_circuit_serialized_v7(
                &witness,
                material.circuit_params.clone(),
                &public,
                &eq_output,
                KagemushaSerializedPublicModeV7::Live,
                KagemushaCircuitBuilderStageV5::Prover(&material.step_ep_break_points),
            )?;
            let instances = vec![public.instance_column::<Fq>(
                case.proof_step_count,
                &material.circuit_params.base,
                KagemushaPastaCycleParityV1::StepEp,
            )?];
            Ok((circuit, instances))
        },
        proof_bound,
        raw_proof_pair_bound,
    )?;
    if pair.step_eq_proof.len() != KAGEMUSHA_SERIALIZED_STEP_PROOF_BYTES_V7
        || pair.step_ep_proof.len() != KAGEMUSHA_SERIALIZED_STEP_PROOF_BYTES_V7
        || pair.step_eq_proof.len() > KAGEMUSHA_SERIALIZED_STEP_PROOF_MAX_BYTES_V7
        || pair.step_ep_proof.len() > KAGEMUSHA_SERIALIZED_STEP_PROOF_MAX_BYTES_V7
        || pair.step_eq_proof.len() + pair.step_ep_proof.len()
            != KAGEMUSHA_SERIALIZED_RAW_PROOF_PAIR_BYTES_V7
        || pair.step_eq_proof.len() + pair.step_ep_proof.len() > raw_proof_pair_bound
    {
        return Err(format!(
            "Kagemusha V7 case {} proof sizes violate exact release caps",
            case.label
        ));
    }
    if pair.step_eq_proving_key_size_bytes != material.step_eq_proving_key_size_bytes
        || pair.step_ep_proving_key_size_bytes != material.step_ep_proving_key_size_bytes
        || pair.step_eq_proving_key_sha256 != material.step_eq_proving_key_sha256
        || pair.step_ep_proving_key_sha256 != material.step_ep_proving_key_sha256
    {
        return Err(format!(
            "Kagemusha V7 case {} proving-key custody identity drifted",
            case.label
        ));
    }
    let mutation_rejections = assert_kagemusha_serialized_live_mutations_fail_v7(material, &pair)?;
    let eq_parent = case
        .public_inputs
        .parent_eq_lineage_accumulator
        .as_ref()
        .map(|wire| wire.to_eq(material.manifest.k))
        .transpose()?;
    let (step_eq_post_proof_fold, step_eq_lineage) =
        super::kagemusha_accumulation::fold_eq_accumulators_v4(
            &material.step_eq_params,
            material.manifest.k,
            pair.verified.step_eq.clone(),
            eq_parent.clone(),
        )?;
    let decided_step_eq_lineage =
        super::kagemusha_accumulation::verify_and_decide_eq_accumulation_v4(
            &material.step_eq_params,
            material.manifest.k,
            pair.verified.step_eq.clone(),
            eq_parent,
            &step_eq_post_proof_fold,
        )?;
    let ep_parent = case
        .public_inputs
        .parent_ep_lineage_accumulator
        .as_ref()
        .map(|wire| wire.to_ep(material.manifest.k))
        .transpose()?;
    let (step_ep_post_proof_fold, step_ep_lineage) =
        super::kagemusha_accumulation::fold_ep_accumulators_v4(
            &material.step_ep_params,
            material.manifest.k,
            pair.verified.step_ep.clone(),
            ep_parent.clone(),
        )?;
    let decided_step_ep_lineage =
        super::kagemusha_accumulation::verify_and_decide_ep_accumulation_v4(
            &material.step_ep_params,
            material.manifest.k,
            pair.verified.step_ep.clone(),
            ep_parent,
            &step_ep_post_proof_fold,
        )?;
    let step_eq_lineage_wire =
        KagemushaIpaAccumulatorWireV4::from_eq(&step_eq_lineage, material.manifest.k)?;
    let step_ep_lineage_wire =
        KagemushaIpaAccumulatorWireV4::from_ep(&step_ep_lineage, material.manifest.k)?;
    if step_eq_lineage_wire
        != KagemushaIpaAccumulatorWireV4::from_eq(&decided_step_eq_lineage, material.manifest.k)?
        || step_ep_lineage_wire
            != KagemushaIpaAccumulatorWireV4::from_ep(
                &decided_step_ep_lineage,
                material.manifest.k,
            )?
    {
        return Err(format!(
            "Kagemusha V7 case {} terminal lineage differs from its encoded carrier",
            case.label
        ));
    }
    let mut carrier = KagemushaSerializedReleaseCarrierV7 {
        proof_step_count: case.proof_step_count,
        public_inputs: case.public_inputs.clone(),
        pair,
        step_eq_lineage: step_eq_lineage_wire,
        step_ep_lineage: step_ep_lineage_wire,
        step_eq_post_proof_fold,
        step_ep_post_proof_fold,
        canonical_bytes: Vec::new(),
        canonical_sha256: [0; 32],
    };
    seal_kagemusha_serialized_release_carrier_v7(material, &mut carrier)?;
    let measurement = KagemushaSerializedReleaseCaseMeasurementV7 {
        label: case.label,
        proof_step_count: case.proof_step_count,
        parent_count: case.public_inputs.parent_count,
        step_eq_proof_bytes: carrier.pair.step_eq_proof.len(),
        step_ep_proof_bytes: carrier.pair.step_ep_proof.len(),
        raw_proof_pair_bytes: carrier.pair.step_eq_proof.len() + carrier.pair.step_ep_proof.len(),
        canonical_carrier_bytes: carrier.canonical_bytes.len(),
        canonical_carrier_sha256: carrier.canonical_sha256,
        public_cells: carrier.pair.step_eq_instances[0].len(),
        eq_coefficients: eq_coefficients.len(),
        ep_coefficients: ep_coefficients.len(),
    };
    drop(witness);
    drop(step_eq_recursion);
    drop(step_ep_recursion);
    drop(parents);
    halo2_proofs::release_allocator_slack();
    Ok(KagemushaSerializedProvedCaseV7 {
        carrier,
        measurement,
        mutation_rejections,
        terminal_ipa_decisions: parent_decisions + 4,
    })
}

#[allow(clippy::too_many_lines)]
fn run_kagemusha_serialized_release_proof_driver_v7(
    material: &KagemushaSerializedReleaseMaterialV7,
    expected_manifest_sha256: [u8; 32],
) -> Result<KagemushaSerializedReleaseProofMeasurementV7, String> {
    use halo2_proofs::{SerdeFormat, poly::commitment::Params as _};

    if material.manifest.sha256()? != expected_manifest_sha256 {
        return Err("Kagemusha V7 driver manifest differs from the trusted expectation".to_owned());
    }
    let mut carriers = Vec::<KagemushaSerializedReleaseCarrierV7>::with_capacity(4);
    let mut measurements = Vec::with_capacity(4);
    let mut mutation_rejections = 0_usize;
    // The final all-zero null pair was already terminally decided once per
    // parity while preparing this material.
    let mut terminal_ipa_decisions = 2_usize;

    let mut init = kagemusha_serialized_init_material_v7(material)?;
    let mut base_case = KagemushaSerializedReleaseProofCaseV7 {
        label: "initialization",
        proof_step_count: 1,
        public_inputs: init.public_inputs.clone(),
        secure: &init.relation.secure,
        output_membership: &init.relation.output_membership,
        parent_bundle_digests: Vec::new(),
        parent_indices: Vec::new(),
    };
    let proved = prove_kagemusha_serialized_release_case_v7(
        material,
        expected_manifest_sha256,
        &mut base_case,
        &carriers,
    )?;
    init.public_inputs = base_case.public_inputs;
    mutation_rejections = mutation_rejections
        .checked_add(proved.mutation_rejections)
        .ok_or_else(|| "Kagemusha V7 mutation total overflowed".to_owned())?;
    terminal_ipa_decisions = terminal_ipa_decisions
        .checked_add(proved.terminal_ipa_decisions)
        .ok_or_else(|| "Kagemusha V7 terminal decision total overflowed".to_owned())?;
    measurements.push(proved.measurement);
    carriers.push(proved.carrier);
    let base_bundle_digest = kagemusha_serialized_carrier_bundle_digest_v7(material, &carriers[0])?;

    let mut siblings =
        kagemusha_serialized_sibling_material_v7(material, &init, base_bundle_digest)?;
    let secure = &siblings.secure;
    let output_membership = &siblings.output_membership;
    for branch in &mut siblings.branches {
        let mut case = KagemushaSerializedReleaseProofCaseV7 {
            label: branch.label,
            proof_step_count: 2,
            public_inputs: branch.public_inputs.clone(),
            secure,
            output_membership,
            parent_bundle_digests: vec![base_bundle_digest],
            parent_indices: vec![0],
        };
        let proved = prove_kagemusha_serialized_release_case_v7(
            material,
            expected_manifest_sha256,
            &mut case,
            &carriers,
        )?;
        branch.public_inputs = case.public_inputs;
        mutation_rejections = mutation_rejections
            .checked_add(proved.mutation_rejections)
            .ok_or_else(|| "Kagemusha V7 mutation total overflowed".to_owned())?;
        terminal_ipa_decisions = terminal_ipa_decisions
            .checked_add(proved.terminal_ipa_decisions)
            .ok_or_else(|| "Kagemusha V7 terminal decision total overflowed".to_owned())?;
        measurements.push(proved.measurement);
        carriers.push(proved.carrier);
    }
    let mut sibling_bundle_digests = [
        kagemusha_serialized_carrier_bundle_digest_v7(material, &carriers[1])?,
        kagemusha_serialized_carrier_bundle_digest_v7(material, &carriers[2])?,
    ];
    if sibling_bundle_digests[0] == sibling_bundle_digests[1] {
        return Err("Kagemusha V7 distinct sibling carrier identities collide".to_owned());
    }
    if sibling_bundle_digests[0] > sibling_bundle_digests[1] {
        sibling_bundle_digests.swap(0, 1);
        siblings.branches.swap(0, 1);
        carriers.swap(1, 2);
        measurements.swap(1, 2);
    }
    for (slot, (branch, carrier)) in siblings.branches.iter().zip(&carriers[1..3]).enumerate() {
        if branch.public_inputs.result_state != carrier.public_inputs.result_state
            || sibling_bundle_digests[slot]
                != kagemusha_serialized_carrier_bundle_digest_v7(material, carrier)?
        {
            return Err("Kagemusha V7 sorted sibling carrier binding drifted".to_owned());
        }
    }

    let mut merge =
        kagemusha_serialized_merge_material_v7(material, &siblings, sibling_bundle_digests)?;
    let mut merge_case = KagemushaSerializedReleaseProofCaseV7 {
        label: "two-parent-merge",
        proof_step_count: 3,
        public_inputs: merge.public_inputs.clone(),
        secure: &merge.secure,
        output_membership: &merge.output_membership,
        parent_bundle_digests: sibling_bundle_digests.to_vec(),
        parent_indices: vec![1, 2],
    };
    let proved = prove_kagemusha_serialized_release_case_v7(
        material,
        expected_manifest_sha256,
        &mut merge_case,
        &carriers,
    )?;
    merge.public_inputs = merge_case.public_inputs;
    mutation_rejections = mutation_rejections
        .checked_add(proved.mutation_rejections)
        .ok_or_else(|| "Kagemusha V7 mutation total overflowed".to_owned())?;
    terminal_ipa_decisions = terminal_ipa_decisions
        .checked_add(proved.terminal_ipa_decisions)
        .ok_or_else(|| "Kagemusha V7 terminal decision total overflowed".to_owned())?;
    measurements.push(proved.measurement);
    carriers.push(proved.carrier);
    if carriers.len() != 4
        || measurements
            .iter()
            .map(|measurement| (measurement.proof_step_count, measurement.parent_count))
            .collect::<Vec<_>>()
            != vec![(1, 0), (2, 1), (2, 1), (3, 2)]
        || merge.statement.proof_step_count != 3
        || merge.statement.current_note.amount.atomic_units
            != KAGEMUSHA_SERIALIZED_FIXTURE_AMOUNT_V7
    {
        return Err("Kagemusha V7 genuine four-node topology drifted".to_owned());
    }
    let absolute_carrier_max =
        usize::try_from(KAGEMUSHA_RECURSIVE_SPEND_PROOF_PAIR_ABSOLUTE_MAX_BYTES_V4)
            .map_err(|_| "Kagemusha V7 absolute carrier bound does not fit usize".to_owned())?;
    for (index, (carrier, measurement)) in carriers.iter().zip(&measurements).enumerate() {
        validate_kagemusha_serialized_release_carrier_seal_v7(material, carrier)?;
        if carrier.canonical_bytes.len() != measurement.canonical_carrier_bytes
            || carrier.canonical_sha256 != measurement.canonical_carrier_sha256
            || measurement.canonical_carrier_sha256 == [0; 32]
            || measurement.canonical_carrier_bytes <= measurement.raw_proof_pair_bytes
            || measurement.canonical_carrier_bytes > absolute_carrier_max
        {
            return Err(format!(
                "Kagemusha V7 case {index} canonical carrier measurement/seal drifted"
            ));
        }
        if carriers[..index]
            .iter()
            .any(|prior| prior.canonical_sha256 == carrier.canonical_sha256)
        {
            return Err("Kagemusha V7 canonical carrier SHA-256 identities collide".to_owned());
        }
    }
    validate_kagemusha_serialized_null_carrier_seal_v7(&material.manifest, &material.null_carrier)?;
    if carriers
        .iter()
        .any(|carrier| carrier.canonical_sha256 == material.null_carrier.canonical_sha256)
    {
        return Err("Kagemusha V7 NullParent/live canonical carrier identities collide".to_owned());
    }
    let maximum_live_carrier_bytes = measurements
        .iter()
        .map(|measurement| measurement.canonical_carrier_bytes)
        .max()
        .ok_or_else(|| "Kagemusha V7 emitted no canonical carriers".to_owned())?;
    if measurements[0].canonical_carrier_bytes >= measurements[1].canonical_carrier_bytes
        || measurements[1..].iter().any(|measurement| {
            measurement.canonical_carrier_bytes != measurements[1].canonical_carrier_bytes
        })
        || material.null_carrier.canonical_bytes.len() <= maximum_live_carrier_bytes
    {
        return Err(
            "Kagemusha V7 base/recursive/null canonical carrier byte shapes drifted".to_owned(),
        );
    }
    let maximum_canonical_carrier_bytes =
        maximum_live_carrier_bytes.max(material.null_carrier.canonical_bytes.len());
    let current_release_max =
        usize::try_from(KAGEMUSHA_RECURSIVE_SPEND_PROOF_PAIR_RELEASE_MAX_BYTES_V4)
            .map_err(|_| "Kagemusha V7 current release bound does not fit usize".to_owned())?;
    let canonical_carriers_fit_current_release_max =
        maximum_canonical_carrier_bytes <= current_release_max;

    let mut eq_params_bytes = Vec::new();
    material
        .step_eq_params
        .write(&mut eq_params_bytes)
        .map_err(|error| format!("failed to encode Kagemusha V7 Eq ParamsIPA: {error}"))?;
    let mut ep_params_bytes = Vec::new();
    material
        .step_ep_params
        .write(&mut ep_params_bytes)
        .map_err(|error| format!("failed to encode Kagemusha V7 Ep ParamsIPA: {error}"))?;
    let vk_bytes = [
        material
            .step_eq_verifying_key
            .to_bytes(SerdeFormat::Processed)
            .len(),
        material
            .step_ep_verifying_key
            .to_bytes(SerdeFormat::Processed)
            .len(),
    ];
    let processed = KagemushaProcessedKeyShapeV4 {
        k: material.manifest.k,
        domain_rows: 1_u32
            .checked_shl(material.manifest.k)
            .ok_or_else(|| "Kagemusha V7 domain rows overflowed".to_owned())?,
        fixed_polynomials: usize::try_from(material.manifest.step_eq_fixed_columns)
            .map_err(|_| "Kagemusha V7 fixed columns do not fit usize".to_owned())?,
        permutation_polynomials: usize::try_from(material.manifest.step_eq_permutation_columns)
            .map_err(|_| "Kagemusha V7 permutation columns do not fit usize".to_owned())?,
        point_bytes: 32,
        scalar_bytes: 32,
    };
    let shape_proving_key_bytes = [
        processed.proving_key_bytes("serialized V7 Eq")?,
        processed.proving_key_bytes("serialized V7 Ep")?,
    ];
    let proving_key_bytes = [
        material.step_eq_proving_key_size_bytes,
        material.step_ep_proving_key_size_bytes,
    ];
    let proving_key_sha256 = [
        material.step_eq_proving_key_sha256,
        material.step_ep_proving_key_sha256,
    ];
    let params_bytes = [eq_params_bytes.len(), ep_params_bytes.len()];
    let conservative_peak_bytes =
        kagemusha_serialized_generation_peak_bound_v7(&material.circuit_params)?;
    if params_bytes != [KAGEMUSHA_SERIALIZED_PARAMS_BYTES_V7; 2]
        || params_bytes.iter().any(|size| {
            u64::try_from(*size).map_or(true, |size| {
                size > KAGEMUSHA_COMPACT_PARAMS_IPA_MAX_BYTES_V5
            })
        })
        || vk_bytes != [KAGEMUSHA_SERIALIZED_VERIFYING_KEY_BYTES_V7; 2]
        || shape_proving_key_bytes != [KAGEMUSHA_SERIALIZED_PROVING_KEY_BYTES_V7; 2]
        || proving_key_bytes != shape_proving_key_bytes
        || proving_key_bytes
            .iter()
            .any(|size| *size > KAGEMUSHA_COMPACT_PROVING_KEY_MAX_BYTES_V5)
        || proving_key_sha256.iter().any(|sha256| *sha256 == [0; 32])
        || conservative_peak_bytes != KAGEMUSHA_SERIALIZED_REVIEWED_PEAK_BYTES_V7
        || conservative_peak_bytes != material.reviewed_peak_bytes
        || conservative_peak_bytes > material.active_memory_limit_bytes
        || material.active_memory_limit_bytes > KAGEMUSHA_GENERATION_REVIEWED_MAX_ESTIMATED_BYTES_V5
        || measurements.iter().any(|measurement| {
            measurement.step_eq_proof_bytes != KAGEMUSHA_SERIALIZED_STEP_PROOF_BYTES_V7
                || measurement.step_ep_proof_bytes != KAGEMUSHA_SERIALIZED_STEP_PROOF_BYTES_V7
                || measurement.step_eq_proof_bytes > KAGEMUSHA_SERIALIZED_STEP_PROOF_MAX_BYTES_V7
                || measurement.step_ep_proof_bytes > KAGEMUSHA_SERIALIZED_STEP_PROOF_MAX_BYTES_V7
                || measurement.raw_proof_pair_bytes != KAGEMUSHA_SERIALIZED_RAW_PROOF_PAIR_BYTES_V7
                || u32::try_from(measurement.raw_proof_pair_bytes).map_or(true, |size| {
                    size > KAGEMUSHA_RECURSIVE_SPEND_PROOF_PAIR_RELEASE_MAX_BYTES_V4
                })
                || measurement.public_cells != KAGEMUSHA_SERIALIZED_PUBLIC_INSTANCE_CELLS_V7
                || u32::try_from(measurement.eq_coefficients).ok()
                    != Some(material.manifest.eq_coefficient_count)
                || u32::try_from(measurement.ep_coefficients).ok()
                    != Some(material.manifest.ep_coefficient_count)
        })
        || mutation_rejections != 40
        || terminal_ipa_decisions != KAGEMUSHA_SERIALIZED_REQUIRED_TERMINAL_IPA_DECISIONS_V7
    {
        return Err("Kagemusha V7 exact release measurements drifted".to_owned());
    }
    Ok(KagemushaSerializedReleaseProofMeasurementV7 {
        null_carrier_bytes: material.null_carrier.canonical_bytes.len(),
        null_carrier_sha256: material.null_carrier.canonical_sha256,
        manifest_sha256: expected_manifest_sha256,
        params_bytes,
        verifying_key_bytes: vk_bytes,
        proving_key_bytes,
        proving_key_sha256,
        conservative_peak_bytes,
        active_memory_limit_bytes: material.active_memory_limit_bytes,
        maximum_canonical_carrier_bytes,
        canonical_carriers_fit_current_release_max,
        cases: measurements,
        mutation_rejections,
        terminal_ipa_decisions,
    })
}

fn promote_kagemusha_serialized_release_proof_v7(
    material: &KagemushaSerializedReleaseMaterialV7,
    expected_manifest_sha256: [u8; 32],
) -> Result<KagemushaSerializedReleaseProofMeasurementV7, String> {
    require_kagemusha_serialized_bridge_release_review_v7()?;
    run_kagemusha_serialized_release_proof_driver_v7(material, expected_manifest_sha256)
}

/// Execute the complete non-shipping V7 release proof from internally derived
/// fixtures.  The caller cannot inject a semantic DAG, Params, keys, or an
/// expected manifest identity.
fn execute_kagemusha_serialized_release_proof_v7()
-> Result<KagemushaSerializedReleaseProofMeasurementV7, String> {
    if KAGEMUSHA_SERIALIZED_BRIDGE_REVIEWED_V7 {
        return Err(
            "Kagemusha V7 non-shipping proof refuses to run after the release gate opens"
                .to_owned(),
        );
    }
    let memory_guard = start_kagemusha_generation_memory_guard_v4(Some(
        KAGEMUSHA_GENERATION_REVIEWED_MAX_ESTIMATED_BYTES_V5,
    ))?;
    // Match the production generator's source-pinned execution envelope: all
    // Params construction, keygen, proving, verification, and measurement run
    // in one disposable Rayon worker.  Halo2's internally bounded MSM window
    // remains the only nested parallelism.
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(KAGEMUSHA_GENERATION_RAYON_THREADS_V5)
        .thread_name(|_| "kagemusha-v7-release-proof".to_owned())
        .build()
        .map_err(|error| format!("failed to build bounded Kagemusha V7 worker pool: {error}"))?;
    pool.install(move || execute_kagemusha_serialized_release_proof_in_pool_v7(memory_guard))
}

fn execute_kagemusha_serialized_release_proof_in_pool_v7(
    memory_guard: KagemushaGenerationMemoryGuardV4,
) -> Result<KagemushaSerializedReleaseProofMeasurementV7, String> {
    let material = prepare_kagemusha_serialized_release_material_v7(
        kagemusha_serialized_release_manifest_template_v7(),
        &memory_guard,
    )?;
    let expected_manifest_sha256 = material.manifest.sha256()?;
    let measurement =
        run_kagemusha_serialized_release_proof_driver_v7(&material, expected_manifest_sha256)?;
    let expected_gate_error = match require_kagemusha_serialized_bridge_release_review_v7() {
        Ok(()) => {
            return Err("Kagemusha V7 release review gate unexpectedly opened".to_owned());
        }
        Err(error) => error,
    };
    match promote_kagemusha_serialized_release_proof_v7(&material, expected_manifest_sha256) {
        Ok(_) => Err("Kagemusha V7 non-shipping proof bypassed the release gate".to_owned()),
        Err(error) if error == expected_gate_error => Ok(measurement),
        Err(error) => Err(format!(
            "Kagemusha V7 promotion failed outside the hard-false review gate: {error}"
        )),
    }
}
