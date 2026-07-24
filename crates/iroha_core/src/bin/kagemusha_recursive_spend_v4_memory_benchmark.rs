//! Measure the non-shipping compact Kagemusha generator under its external guard.
//!
//! This diagnostic executes the complete generate, bootstrap, live-prove, and
//! terminal-verification lifecycle. Proving keys are streamed into anonymous
//! files, and the process emits only validated byte counts; it cannot frame or
//! publish candidate or release artifacts.

use std::{
    env,
    error::Error,
    ffi::OsStr,
    fs::File,
    io::{self, Write as _},
};

use iroha_core::zk::kagemusha_v2::{
    KagemushaGeneratedParityArtifactsV4, claim_kagemusha_generation_supervisor_permit_v4,
    generate_kagemusha_pasta_cycle_artifacts_v4, validate_kagemusha_proof_pair_measurement_v4,
    validate_kagemusha_step_bootstrap_payload_v4,
};
use iroha_data_model::offline::{
    KAGEMUSHA_RECURSIVE_SPEND_PROOF_PAIR_ABSOLUTE_MAX_BYTES_V4,
    KAGEMUSHA_STEP_CIRCUIT_MINIMUM_K_V4, KAGEMUSHA_STEP_CIRCUIT_MINIMUM_UNUSABLE_ROWS_V4,
    KAGEMUSHA_STEP_CIRCUIT_PARAMS_VERSION_V4, KAGEMUSHA_STEP_PROOF_ABSOLUTE_MAX_BYTES_V4,
    KagemushaPastaCycleParityV1, KagemushaPastaPublicLayoutV4, KagemushaStepCircuitParamsV4,
};

const SUBCOMMAND: &str = "measure-compact-k16";
const EXPECTED_PARAMETERS_BYTES: usize = 4_194_372;
const EXPECTED_VERIFYING_KEY_BYTES: usize = 682;
const EXPECTED_PROVING_KEY_BYTES: u64 = 94_372_718;

fn benchmark_error(message: impl Into<String>) -> io::Error {
    io::Error::other(message.into())
}

fn compact_k16_params() -> Result<KagemushaStepCircuitParamsV4, io::Error> {
    let k = KAGEMUSHA_STEP_CIRCUIT_MINIMUM_K_V4;
    let layout = KagemushaPastaPublicLayoutV4::for_ipa_round_count(k)
        .map_err(|error| benchmark_error(format!("compact public layout is invalid: {error}")))?;
    let params = KagemushaStepCircuitParamsV4 {
        version: KAGEMUSHA_STEP_CIRCUIT_PARAMS_VERSION_V4,
        k,
        num_advice_per_phase: vec![8],
        num_lookup_advice_per_phase: vec![1],
        num_fixed: 1,
        lookup_bits: k - 1,
        num_instance_columns: 1,
        public_input_limbs: layout.instance_column_limbs,
        minimum_unusable_rows: KAGEMUSHA_STEP_CIRCUIT_MINIMUM_UNUSABLE_ROWS_V4,
        max_parent_proof_bytes: KAGEMUSHA_STEP_PROOF_ABSOLUTE_MAX_BYTES_V4,
    };
    let validated_layout = params
        .validate()
        .map_err(|error| benchmark_error(format!("compact circuit profile is invalid: {error}")))?;
    if validated_layout != layout || layout.instance_column_limbs != 64 {
        return Err(benchmark_error(
            "compact circuit profile does not select the fixed 64-limb public layout",
        ));
    }
    Ok(params)
}

fn validate_parity(
    label: &str,
    parity: KagemushaPastaCycleParityV1,
    artifacts: &KagemushaGeneratedParityArtifactsV4,
    proving_key_sink: &File,
) -> Result<usize, io::Error> {
    artifacts.circuit_params.validate().map_err(|error| {
        benchmark_error(format!("generated {label} profile is invalid: {error}"))
    })?;
    if artifacts.circuit_params.k != KAGEMUSHA_STEP_CIRCUIT_MINIMUM_K_V4
        || artifacts.parameters.len() != EXPECTED_PARAMETERS_BYTES
        || artifacts.verifying_key.len() != EXPECTED_VERIFYING_KEY_BYTES
        || artifacts.proving_key_size_bytes != EXPECTED_PROVING_KEY_BYTES
        || proving_key_sink
            .metadata()
            .map_err(|error| {
                benchmark_error(format!("failed to inspect {label} PK sink: {error}"))
            })?
            .len()
            != EXPECTED_PROVING_KEY_BYTES
    {
        return Err(benchmark_error(format!(
            "generated {label} artifacts do not match the reviewed compact-k16 byte geometry"
        )));
    }
    if artifacts.compiled_protocol_structure_sha256 == [0; 32]
        || artifacts.step_proof_size_bytes == 0
        || artifacts.step_proof_size_bytes > KAGEMUSHA_STEP_PROOF_ABSOLUTE_MAX_BYTES_V4
        || artifacts.step_proof_size_bytes != artifacts.circuit_params.max_parent_proof_bytes
    {
        return Err(benchmark_error(format!(
            "generated {label} calibration metadata is inconsistent"
        )));
    }
    let measured = validate_kagemusha_step_bootstrap_payload_v4(
        &artifacts.bootstrap_witness,
        &artifacts.circuit_params,
        parity,
        artifacts.compiled_protocol_structure_sha256,
    )
    .map_err(|error| benchmark_error(format!("generated {label} bootstrap is invalid: {error}")))?;
    if u32::try_from(measured) != Ok(artifacts.step_proof_size_bytes) {
        return Err(benchmark_error(format!(
            "generated {label} bootstrap measurement differs from its calibrated proof size"
        )));
    }
    Ok(measured)
}

fn run_measurement() -> Result<(), Box<dyn Error>> {
    // Claim the one-shot inherited capability before allocating either Pasta
    // parameter set or opening even the anonymous proving-key sinks.
    let supervisor_permit = claim_kagemusha_generation_supervisor_permit_v4()
        .map_err(|error| benchmark_error(format!("resource guard is unavailable: {error}")))?;
    let params = compact_k16_params()?;
    let mut step_eq_proving_key_sink = tempfile::tempfile()?;
    let mut step_ep_proving_key_sink = tempfile::tempfile()?;
    let generated = generate_kagemusha_pasta_cycle_artifacts_v4(
        params.clone(),
        params,
        supervisor_permit,
        &mut step_eq_proving_key_sink,
        &mut step_ep_proving_key_sink,
    )
    .map_err(|error| benchmark_error(format!("compact generation failed: {error}")))?;
    step_eq_proving_key_sink.flush()?;
    step_ep_proving_key_sink.flush()?;

    let measured_eq = validate_parity(
        "Eq",
        KagemushaPastaCycleParityV1::StepEq,
        &generated.step_eq,
        &step_eq_proving_key_sink,
    )?;
    let measured_ep = validate_parity(
        "Ep",
        KagemushaPastaCycleParityV1::StepEp,
        &generated.step_ep,
        &step_ep_proving_key_sink,
    )?;
    if generated.step_eq.compiled_protocol_structure_sha256
        == generated.step_ep.compiled_protocol_structure_sha256
    {
        return Err(benchmark_error("generated Eq/Ep protocol structures collide").into());
    }

    let pair_bytes = u32::try_from(generated.measured_live_pair_bytes.len())
        .map_err(|_| benchmark_error("measured proof-pair length does not fit u32"))?;
    let step_bytes = generated
        .step_eq
        .step_proof_size_bytes
        .checked_add(generated.step_ep.step_proof_size_bytes)
        .ok_or_else(|| benchmark_error("measured Step-proof byte sum overflowed"))?;
    if pair_bytes <= step_bytes
        || pair_bytes > KAGEMUSHA_RECURSIVE_SPEND_PROOF_PAIR_ABSOLUTE_MAX_BYTES_V4
    {
        return Err(
            benchmark_error("measured proof pair is outside its fixed byte corridor").into(),
        );
    }
    let measured_pair = validate_kagemusha_proof_pair_measurement_v4(
        &generated.measured_live_pair_bytes,
        &generated.step_eq.circuit_params,
        &generated.step_ep.circuit_params,
        pair_bytes,
    )
    .map_err(|error| benchmark_error(format!("generated live proof pair is invalid: {error}")))?;
    if measured_pair != generated.measured_live_pair_bytes.len() {
        return Err(benchmark_error("proof-pair validator returned a different byte count").into());
    }

    println!("benchmark=NON_SHIPPING");
    println!("eq_parameters_bytes={}", generated.step_eq.parameters.len());
    println!(
        "eq_proving_key_bytes={}",
        generated.step_eq.proving_key_size_bytes
    );
    println!(
        "eq_verifying_key_bytes={}",
        generated.step_eq.verifying_key.len()
    );
    println!(
        "eq_bootstrap_bytes={}",
        generated.step_eq.bootstrap_witness.len()
    );
    println!("eq_step_proof_bytes={measured_eq}");
    println!("ep_parameters_bytes={}", generated.step_ep.parameters.len());
    println!(
        "ep_proving_key_bytes={}",
        generated.step_ep.proving_key_size_bytes
    );
    println!(
        "ep_verifying_key_bytes={}",
        generated.step_ep.verifying_key.len()
    );
    println!(
        "ep_bootstrap_bytes={}",
        generated.step_ep.bootstrap_witness.len()
    );
    println!("ep_step_proof_bytes={measured_ep}");
    println!("proof_pair_bytes={measured_pair}");
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = env::args_os().skip(1);
    match (args.next(), args.next()) {
        (Some(subcommand), None) if subcommand == OsStr::new(SUBCOMMAND) => run_measurement(),
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("usage: kagemusha_recursive_spend_v4_memory_benchmark {SUBCOMMAND}"),
        )
        .into()),
    }
}
