//! Build and sign a Taira localnet genesis overlay that seeds Kaigi relay metadata.

use std::{fs, path::PathBuf, str::FromStr};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use clap::Parser;
use color_eyre::{
    Result,
    eyre::{WrapErr, ensure},
};
use iroha_crypto::{Algorithm, KeyPair, PublicKey};
use iroha_data_model::{
    account::{AccountId, address::ChainDiscriminantGuard},
    domain::DomainId,
    isi::SetKeyValue,
    kaigi::{KaigiId, KaigiRelayFeedback, KaigiRelayHealthStatus, KaigiRelayRegistration},
    name::Name,
};
use iroha_genesis::RawGenesisTransaction;
use iroha_primitives::json::Json;

#[derive(Parser, Debug)]
struct Args {
    /// Base genesis JSON manifest to overlay.
    #[arg(long)]
    genesis: PathBuf,
    /// Output path for the signed genesis `.nrt`.
    #[arg(long)]
    out_file: PathBuf,
    /// Deterministic seed used for the genesis signing key.
    #[arg(long)]
    seed: String,
    /// Public key of the account that will appear as the relay-health reporter.
    #[arg(long)]
    host_public_key: String,
    /// Relay specs in the format `<PUBLIC_KEY>:<HPKE_KEY_B64>:<BANDWIDTH_CLASS>`.
    #[arg(long = "relay-spec", required = true)]
    relay_specs: Vec<String>,
    /// Domain that stores relay metadata.
    #[arg(long, default_value = "nexus")]
    relay_domain: String,
    /// Domain recorded in the seeded feedback's Kaigi call id.
    #[arg(long, default_value = "wonderland")]
    call_domain: String,
    /// Call name recorded in the seeded feedback's Kaigi call id.
    #[arg(long, default_value = "taira-relay-bootstrap")]
    call_name: String,
    /// Millisecond timestamp recorded in seeded health feedback.
    #[arg(long, default_value_t = 1_890_864_000_000u64)]
    reported_at_ms: u64,
    /// Human-readable note recorded in seeded relay-health feedback.
    #[arg(long, default_value = "Seeded in Taira local genesis")]
    notes: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RelaySpec {
    public_key: PublicKey,
    hpke_public_key_b64: String,
    bandwidth_class: u8,
}

impl RelaySpec {
    fn parse(raw: &str) -> Result<Self> {
        let mut parts = raw.splitn(3, ':');
        let public_key = parts
            .next()
            .ok_or_else(|| color_eyre::eyre::eyre!("relay spec missing public key"))?;
        let hpke_public_key_b64 = parts
            .next()
            .ok_or_else(|| color_eyre::eyre::eyre!("relay spec missing HPKE key"))?;
        let bandwidth_class = parts
            .next()
            .ok_or_else(|| color_eyre::eyre::eyre!("relay spec missing bandwidth class"))?;
        let public_key = PublicKey::from_str(public_key)
            .wrap_err("failed to parse relay public key from relay spec")?;
        let bandwidth_class = bandwidth_class
            .parse::<u8>()
            .wrap_err("failed to parse relay bandwidth class from relay spec")?;
        ensure!(
            !hpke_public_key_b64.trim().is_empty(),
            "relay spec HPKE key must not be empty"
        );
        ensure!(
            bandwidth_class > 0,
            "relay spec bandwidth class must be non-zero"
        );
        Ok(Self {
            public_key,
            hpke_public_key_b64: hpke_public_key_b64.to_owned(),
            bandwidth_class,
        })
    }
}

fn registration_key(public_key: &PublicKey) -> Result<Name> {
    Name::from_str(&format!("kaigi_relay__{public_key}")).wrap_err("invalid relay metadata key")
}

fn feedback_key(public_key: &PublicKey) -> Result<Name> {
    Name::from_str(&format!("kaigi_relay_feedback__{public_key}"))
        .wrap_err("invalid relay feedback key")
}

fn relay_registration(spec: &RelaySpec) -> KaigiRelayRegistration {
    KaigiRelayRegistration {
        relay_id: AccountId::new(spec.public_key.clone()),
        hpke_public_key: BASE64_STANDARD
            .decode(&spec.hpke_public_key_b64)
            .expect("validated HPKE base64 should decode"),
        bandwidth_class: spec.bandwidth_class,
    }
}

fn relay_feedback(
    spec: &RelaySpec,
    host: &AccountId,
    call_id: &KaigiId,
    reported_at_ms: u64,
    notes: &str,
) -> KaigiRelayFeedback {
    KaigiRelayFeedback {
        relay_id: AccountId::new(spec.public_key.clone()),
        call: call_id.clone(),
        reported_by: host.clone(),
        status: KaigiRelayHealthStatus::Healthy,
        reported_at_ms,
        notes: Some(notes.to_owned()),
    }
}

fn append_kaigi_overlay(
    manifest: RawGenesisTransaction,
    relay_domain: &DomainId,
    host: &AccountId,
    call_id: &KaigiId,
    relay_specs: &[RelaySpec],
    reported_at_ms: u64,
    notes: &str,
) -> Result<RawGenesisTransaction> {
    let mut builder = manifest.into_builder().next_transaction();
    for relay in relay_specs {
        let registration = relay_registration(relay);
        builder = builder.append_instruction(SetKeyValue::domain(
            relay_domain.clone(),
            registration_key(&relay.public_key)?,
            Json::try_new(registration).wrap_err("failed to serialize relay registration JSON")?,
        ));
    }
    for relay in relay_specs {
        let feedback = relay_feedback(relay, host, call_id, reported_at_ms, notes);
        builder = builder.append_instruction(SetKeyValue::domain(
            relay_domain.clone(),
            feedback_key(&relay.public_key)?,
            Json::try_new(feedback).wrap_err("failed to serialize relay feedback JSON")?,
        ));
    }
    Ok(builder.build_raw())
}

fn run(args: &Args) -> Result<()> {
    ensure!(
        !args.relay_specs.is_empty(),
        "at least one --relay-spec is required"
    );
    let relay_specs = args
        .relay_specs
        .iter()
        .map(|raw| RelaySpec::parse(raw))
        .collect::<Result<Vec<_>>>()?;
    let manifest = RawGenesisTransaction::from_path(&args.genesis)
        .wrap_err("failed to load base genesis manifest")?;
    let _chain_discriminant = ChainDiscriminantGuard::enter(manifest.chain_discriminant());
    let host_public_key =
        PublicKey::from_str(&args.host_public_key).wrap_err("failed to parse host public key")?;
    let host = AccountId::new(host_public_key);
    let relay_domain = DomainId::from_str(&args.relay_domain).wrap_err("invalid relay domain")?;
    let call_id = KaigiId::new(
        DomainId::from_str(&args.call_domain).wrap_err("invalid call domain")?,
        Name::from_str(&args.call_name).wrap_err("invalid call name")?,
    );
    let signed = append_kaigi_overlay(
        manifest,
        &relay_domain,
        &host,
        &call_id,
        &relay_specs,
        args.reported_at_ms,
        &args.notes,
    )?
    .build_and_sign(&KeyPair::from_seed(
        args.seed.as_bytes().to_vec(),
        Algorithm::Ed25519,
    ))
    .wrap_err("failed to sign Kaigi overlay genesis")?;

    let framed = signed
        .0
        .encode_wire()
        .wrap_err("failed to frame signed genesis as Norito wire bytes")?;
    fs::write(&args.out_file, framed).wrap_err("failed to write signed genesis output")?;

    println!(
        "Wrote signed Taira Kaigi overlay with {} relays to {}",
        relay_specs.len(),
        args.out_file.display()
    );
    Ok(())
}

fn main() -> Result<()> {
    color_eyre::install()?;
    run(&Args::parse())
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_genesis::GenesisBuilder;

    #[test]
    fn relay_spec_parses_expected_fields() {
        let raw = "ea0130B99B89AD5D2F51D17AB69D32BC3A44C2CC5FF65E28590022B972148AD4DF00712FEC4EFF5BC6B3AEF33ABCF18F5CAD5B:K4NiAXqV5L1V3aD+/9NItPlFhEtm3qD4Q4K/1M8jewQ=:3";
        let parsed = RelaySpec::parse(raw).expect("relay spec should parse");
        assert_eq!(
            parsed.hpke_public_key_b64,
            "K4NiAXqV5L1V3aD+/9NItPlFhEtm3qD4Q4K/1M8jewQ="
        );
        assert_eq!(parsed.bandwidth_class, 3);
        assert_eq!(
            parsed.public_key.to_string(),
            "ea0130B99B89AD5D2F51D17AB69D32BC3A44C2CC5FF65E28590022B972148AD4DF00712FEC4EFF5BC6B3AEF33ABCF18F5CAD5B"
        );
    }

    #[test]
    fn relay_metadata_keys_use_public_key_suffix() {
        let public_key = PublicKey::from_str(
            "ea0130B4A704CBEADF686CAECDAF705102C9902CFED8B71016906F6D724D0BB7F04DE540F29585B7FB8B46962FB70D0AD97249",
        )
        .expect("public key");
        assert_eq!(
            registration_key(&public_key)
                .expect("registration key")
                .to_string(),
            "kaigi_relay__ea0130B4A704CBEADF686CAECDAF705102C9902CFED8B71016906F6D724D0BB7F04DE540F29585B7FB8B46962FB70D0AD97249"
        );
        assert_eq!(
            feedback_key(&public_key).expect("feedback key").to_string(),
            "kaigi_relay_feedback__ea0130B4A704CBEADF686CAECDAF705102C9902CFED8B71016906F6D724D0BB7F04DE540F29585B7FB8B46962FB70D0AD97249"
        );
    }

    #[test]
    fn manifest_chain_discriminant_scopes_overlay_account_literals() {
        let manifest = GenesisBuilder::new_without_executor(
            "iroha:test:kaigi-taira".parse().expect("chain id"),
            PathBuf::from("."),
        )
        .build_raw()
        .with_chain_discriminant(369);
        let _chain_discriminant = ChainDiscriminantGuard::enter(manifest.chain_discriminant());
        let public_key = PublicKey::from_str(
            "ea0130B99B89AD5D2F51D17AB69D32BC3A44C2CC5FF65E28590022B972148AD4DF00712FEC4EFF5BC6B3AEF33ABCF18F5CAD5B",
        )
        .expect("public key");
        let relay = relay_registration(&RelaySpec {
            public_key,
            hpke_public_key_b64: "K4NiAXqV5L1V3aD+/9NItPlFhEtm3qD4Q4K/1M8jewQ=".to_string(),
            bandwidth_class: 3,
        });
        let value = Json::try_new(relay).expect("serialize relay registration");
        assert!(
            value.get().contains("test"),
            "expected Taira/testnet prefix in relay registration JSON: {}",
            value.get()
        );
    }
}
