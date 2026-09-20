//! One bounded signed-genesis readiness probe; callers own retries and deadlines.
use crate::{CliOutputFormat, RunContext};
use base64::Engine as _;
use eyre::{Result, ensure};
use iroha::{
    client::GenesisFinalityReadiness,
    crypto::{Hash, HashOf, PublicKey},
    data_model::{
        block::{
            BlockHeader,
            consensus_v2::{HeightContext, HeightContextId},
        },
        id::NetworkId,
    },
};
use iroha_model_base::peer::PeerId;
use std::time::{Duration, Instant};

const MAX_OUTPUT_BYTES: usize = 24 * 1024 * 1024;
const MAX_ATTESTATION_BYTES: usize = 16 * 1024 * 1024;

/// Original process identities and one finite overall HTTP probe budget.
#[derive(clap::Args, Debug)]
pub struct Args {
    /// Fresh unpredictable 32-byte challenge as exactly 64 lowercase hexadecimal characters.
    #[arg(long)]
    challenge: String,
    /// BLS public key independently retained for this original validator process.
    #[arg(long)]
    node_public_key: PublicKey,
    /// Canonical genesis hash retained from original generated inputs.
    #[arg(long)]
    genesis_hash: Hash,
    /// Canonical original height-one context hash; never obtained from the response.
    #[arg(long)]
    context_id: Hash,
    /// Total HTTP budget for compatibility and attestation, within the launcher's deadline.
    #[arg(long, value_parser = clap::value_parser!(u64).range(1..=60_000))]
    request_timeout_ms: u64,
}

#[derive(norito::derive::JsonSerialize)]
struct Report {
    version: u8,
    state: &'static str,
    reason: Option<&'static str>,
    challenge: String,
    node_id: PeerId,
    network_id: NetworkId,
    genesis_hash: HashOf<BlockHeader>,
    context_id: HeightContextId,
    attestation_norito_base64: Option<String>,
}

fn parse_challenge(value: &str) -> Result<[u8; 32]> {
    ensure!(
        value.len() == 64
            && value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)),
        "challenge must be exactly 64 lowercase hexadecimal characters"
    );
    let mut challenge = [0; 32];
    hex::decode_to_slice(value, &mut challenge)?;
    ensure!(
        challenge.iter().any(|byte| *byte != 0),
        "challenge must be nonzero"
    );
    Ok(challenge)
}

fn report(args: &Args, network_id: NetworkId, outcome: GenesisFinalityReadiness) -> Result<Report> {
    let (state, reason, attestation_norito_base64) = match outcome {
        GenesisFinalityReadiness::Ready(attestation) => {
            let bytes = norito::to_bytes(attestation.as_ref())?;
            ensure!(
                bytes.len() <= MAX_ATTESTATION_BYTES,
                "verified attestation exceeds the canonical wire limit"
            );
            (
                "ready",
                None,
                Some(base64::engine::general_purpose::STANDARD.encode(bytes)),
            )
        }
        GenesisFinalityReadiness::NotReady(reason) => {
            (reason.readiness_state(), Some(reason.as_str()), None)
        }
    };
    Ok(Report {
        version: 1,
        state,
        reason,
        challenge: args.challenge.clone(),
        node_id: PeerId::new(args.node_public_key.clone()),
        network_id,
        genesis_hash: HashOf::from_untyped_unchecked(args.genesis_hash),
        context_id: HeightContextId(HashOf::<HeightContext>::from_untyped_unchecked(
            args.context_id,
        )),
        attestation_norito_base64,
    })
}

fn render_report(report: &Report) -> Result<String> {
    let output = norito::json::to_json(report)?;
    ensure!(
        output.len() < MAX_OUTPUT_BYTES,
        "genesis readiness output exceeds 24 MiB including its newline"
    );
    Ok(output)
}

pub(super) fn run(context: &mut impl RunContext, args: Args) -> Result<()> {
    ensure!(
        context.output_format() == CliOutputFormat::Json,
        "genesis-readiness requires --output-format json"
    );
    ensure!(
        (1..=60_000).contains(&args.request_timeout_ms),
        "request timeout must be 1..=60000 milliseconds"
    );
    let deadline = Instant::now() + Duration::from_millis(args.request_timeout_ms);
    let challenge = parse_challenge(&args.challenge)?;
    let node_id = PeerId::new(args.node_public_key.clone());
    let genesis_hash = HashOf::<BlockHeader>::from_untyped_unchecked(args.genesis_hash);
    let context_id = HeightContextId(HashOf::<HeightContext>::from_untyped_unchecked(
        args.context_id,
    ));
    let network_id = context.config().network_id;
    let mut builder = context.client_from_config()?.to_builder();
    builder.torii_request_timeout = Duration::from_millis(args.request_timeout_ms);
    let client = builder.build()?;
    let outcome = client.poll_genesis_finality_attestation(
        challenge,
        &node_id,
        network_id,
        genesis_hash,
        context_id,
        deadline,
    )?;
    context.println(render_report(&report(&args, network_id, outcome)?)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser as _;
    use iroha::crypto::{Algorithm, KeyPair};
    use iroha_torii_shared::bridge_attestation::FinalityAttestationFailureReason as Reason;
    #[derive(clap::Parser)]
    struct Probe {
        #[command(flatten)]
        args: Args,
    }
    fn args() -> Args {
        Args {
            challenge: "47".repeat(32),
            node_public_key: KeyPair::try_from_seed(vec![14; 32], Algorithm::BlsNormal)
                .expect("BLS fixture")
                .public_key()
                .clone(),
            genesis_hash: Hash::new(b"original genesis"),
            context_id: Hash::new(b"original context"),
            request_timeout_ms: 1000,
        }
    }
    #[test]
    fn challenge_rejects_zero_noncanonical_and_truncated_inputs() {
        assert_eq!(parse_challenge(&"47".repeat(32)).unwrap(), [71; 32]);
        for value in [
            "00".repeat(32),
            "AA".repeat(32),
            "4g".repeat(32),
            "47".repeat(31),
            "47".repeat(33),
            format!(" {}", "47".repeat(32)),
        ] {
            assert!(parse_challenge(&value).is_err(), "{value:?}");
        }
    }
    #[test]
    fn cli_requires_finite_timeout_and_all_independent_anchors() {
        let original = args();
        let argv = vec![
            "probe".to_owned(),
            "--challenge".to_owned(),
            original.challenge.clone(),
            "--node-public-key".to_owned(),
            original.node_public_key.to_string(),
            "--genesis-hash".to_owned(),
            original.genesis_hash.to_string(),
            "--context-id".to_owned(),
            original.context_id.to_string(),
            "--request-timeout-ms".to_owned(),
            "1000".to_owned(),
        ];
        let parsed = Probe::try_parse_from(argv.clone()).expect("complete probe");
        assert_eq!(parsed.args.context_id, original.context_id);
        for value in ["0", "60001"] {
            let mut changed = argv.clone();
            *changed.last_mut().unwrap() = value.to_owned();
            assert!(Probe::try_parse_from(changed).is_err());
        }
        for index in [1, 3, 5, 7, 9] {
            let mut changed = argv.clone();
            changed.drain(index..index + 2);
            assert!(Probe::try_parse_from(changed).is_err());
        }
    }
    #[test]
    fn report_never_claims_ready_or_emits_evidence_for_failure() {
        let args = args();
        let network =
            NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(args.genesis_hash));
        for (reason, state) in [
            (Reason::ConsensusUninitialized, "pending"),
            (Reason::GenesisUncommitted, "pending"),
            (Reason::RestartRequired, "restart_required"),
            (Reason::TipChanged, "conflict"),
            (Reason::ConflictingState, "conflict"),
            (Reason::FinalityUnavailable, "unavailable"),
            (Reason::InternalFailure, "unavailable"),
        ] {
            let value = report(&args, network, GenesisFinalityReadiness::NotReady(reason)).unwrap();
            assert_eq!(value.state, state);
            assert!(value.attestation_norito_base64.is_none());
            assert_eq!(value.challenge, args.challenge);
            assert_eq!(value.network_id, network);
            let output = render_report(&value).unwrap();
            assert!(output.contains("\"attestation_norito_base64\":null"));
            assert!(output.len() < MAX_OUTPUT_BYTES);
        }
    }
    #[test]
    fn renderer_rejects_output_at_its_hard_limit() {
        let args = args();
        let network =
            NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(args.genesis_hash));
        let mut value = report(
            &args,
            network,
            GenesisFinalityReadiness::NotReady(Reason::GenesisUncommitted),
        )
        .unwrap();
        value.challenge = "a".repeat(MAX_OUTPUT_BYTES);
        assert!(render_report(&value).is_err());
    }
}
