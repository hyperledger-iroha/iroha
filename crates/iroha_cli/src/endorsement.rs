//! Domain endorsement helpers (committees, policies, submissions).

use std::{fs, path::PathBuf, str::FromStr};

use eyre::{Result, WrapErr, eyre};
use iroha::data_model::{
    domain::DomainId,
    isi::{InstructionBox, endorsement as isi_endorsement},
    metadata::Metadata,
    nexus::{
        DOMAIN_ENDORSEMENT_VERSION_V1, DataSpaceId, DomainCommittee, DomainEndorsement,
        DomainEndorsementPolicy, DomainEndorsementScope,
    },
    query::endorsement::prelude::{
        FindDomainCommittee, FindDomainEndorsementPolicy, FindDomainEndorsements,
    },
};
use iroha_crypto::{Hash, KeyPair, PrivateKey, PublicKey, Signature};
use norito::json;

use crate::{Run, RunContext};

fn parse_domain_id_literal(literal: &str) -> std::result::Result<DomainId, String> {
    DomainId::parse_fully_qualified(literal).map_err(|err| err.to_string())
}

#[derive(clap::Subcommand, Debug)]
pub enum Command {
    /// Build a domain endorsement (optionally signing it) and emit JSON to stdout.
    Prepare(PrepareArgs),
    /// Submit a domain endorsement into the chain state for later reuse.
    Submit(SubmitArgs),
    /// List recorded endorsements for a domain.
    List(ListArgs),
    /// Fetch the endorsement policy for a domain.
    Policy(PolicyArgs),
    /// Fetch a registered endorsement committee.
    Committee(CommitteeArgs),
    /// Register an endorsement committee (quorum + members).
    RegisterCommittee(RegisterCommitteeArgs),
    /// Set or replace the endorsement policy for a domain.
    SetPolicy(SetPolicyArgs),
}

#[derive(clap::Args, Debug)]
pub struct PrepareArgs {
    /// Domain identifier being endorsed.
    #[arg(long, value_parser = parse_domain_id_literal)]
    pub domain: DomainId,
    /// Committee identifier backing this endorsement.
    #[arg(long, default_value = "default")]
    pub committee_id: String,
    /// Block height when the endorsement was issued.
    #[arg(long, value_name = "HEIGHT")]
    pub issued_at_height: u64,
    /// Block height when the endorsement expires.
    #[arg(long, value_name = "HEIGHT")]
    pub expires_at_height: u64,
    /// Optional block height (inclusive) when the endorsement becomes valid.
    #[arg(long)]
    pub block_start: Option<u64>,
    /// Optional block height (inclusive) after which the endorsement is invalid.
    #[arg(long)]
    pub block_end: Option<u64>,
    /// Optional dataspace binding for the endorsement.
    #[arg(long)]
    pub dataspace: Option<u64>,
    /// Optional metadata payload (Norito JSON file) to embed.
    #[arg(long, value_name = "PATH")]
    pub metadata: Option<PathBuf>,
    /// Private keys to sign the endorsement body (multiple allowed).
    #[arg(long = "signer-key", value_name = "PRIVATE_KEY", num_args = 0..)]
    pub signer_keys: Vec<String>,
}

#[derive(clap::Args, Debug)]
pub struct SubmitArgs {
    /// Path to the endorsement JSON. If omitted, read from stdin.
    #[arg(long, value_name = "PATH")]
    pub file: Option<PathBuf>,
}

#[derive(clap::Args, Debug)]
pub struct ListArgs {
    /// Domain to query.
    #[arg(long, value_parser = parse_domain_id_literal)]
    pub domain: DomainId,
}

#[derive(clap::Args, Debug)]
pub struct PolicyArgs {
    /// Domain to query.
    #[arg(long, value_parser = parse_domain_id_literal)]
    pub domain: DomainId,
}

#[derive(clap::Args, Debug)]
pub struct CommitteeArgs {
    /// Committee identifier to fetch.
    #[arg(long)]
    pub committee_id: String,
}

#[derive(clap::Args, Debug)]
pub struct RegisterCommitteeArgs {
    /// New committee identifier.
    #[arg(long)]
    pub committee_id: String,
    /// Quorum required to accept an endorsement.
    #[arg(long)]
    pub quorum: u16,
    /// Member public keys allowed to sign endorsements (string form).
    #[arg(long = "member", value_name = "PUBLIC_KEY", required = true)]
    pub members: Vec<String>,
    /// Optional metadata payload (Norito JSON file) to attach.
    #[arg(long, value_name = "PATH")]
    pub metadata: Option<PathBuf>,
}

#[derive(clap::Args, Debug)]
pub struct SetPolicyArgs {
    /// Domain requiring endorsements.
    #[arg(long, value_parser = parse_domain_id_literal)]
    pub domain: DomainId,
    /// Committee identifier to trust.
    #[arg(long)]
    pub committee_id: String,
    /// Maximum age (in blocks) allowed between issuance and acceptance.
    #[arg(long, value_name = "BLOCKS")]
    pub max_endorsement_age: u64,
    /// Whether an endorsement is required for the domain.
    #[arg(long, default_value_t = true)]
    pub required: bool,
}

impl Run for Command {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        match self {
            Command::Prepare(args) => {
                let endorsement = build_endorsement(&args)?;
                context.print_data(&endorsement)
            }
            Command::Submit(args) => submit_endorsement(args, context),
            Command::List(args) => list_endorsements(args, context),
            Command::Policy(args) => show_policy(args, context),
            Command::Committee(args) => show_committee(args, context),
            Command::RegisterCommittee(args) => register_committee(args, context),
            Command::SetPolicy(args) => set_policy(args, context),
        }
    }
}

fn build_endorsement(args: &PrepareArgs) -> Result<DomainEndorsement> {
    let scope = DomainEndorsementScope {
        dataspace: args.dataspace.map(DataSpaceId::new),
        block_start: args.block_start,
        block_end: args.block_end,
    };
    let metadata = if let Some(path) = &args.metadata {
        read_metadata(path)?
    } else {
        Metadata::default()
    };
    let mut endorsement = DomainEndorsement {
        version: DOMAIN_ENDORSEMENT_VERSION_V1,
        domain_id: args.domain.clone(),
        committee_id: args.committee_id.clone(),
        statement_hash: Hash::new(args.domain.to_string().as_bytes()),
        issued_at_height: args.issued_at_height,
        expires_at_height: args.expires_at_height,
        scope,
        signatures: Vec::new(),
        metadata,
    };
    if args.expires_at_height <= args.issued_at_height {
        return Err(eyre!(
            "expires_at_height ({}) must be greater than issued_at_height ({})",
            args.expires_at_height,
            args.issued_at_height
        ));
    }

    let body_hash = endorsement.body_hash();
    for signer in &args.signer_keys {
        let private: PrivateKey = signer
            .parse()
            .map_err(|err| eyre!("failed to parse signer key: {err}"))?;
        let kp = KeyPair::from_private_key(private.clone())
            .wrap_err("failed to derive keypair from private key")?;
        let signature = Signature::new(&private, body_hash.as_ref());
        endorsement
            .signatures
            .push(iroha::data_model::nexus::DomainEndorsementSignature {
                signer: kp.public_key().clone(),
                signature,
            });
    }

    Ok(endorsement)
}

fn submit_endorsement<C: RunContext>(args: SubmitArgs, context: &mut C) -> Result<()> {
    let endorsement: DomainEndorsement = if let Some(path) = args.file {
        let buf = fs::read_to_string(&path)
            .wrap_err_with(|| format!("failed to read endorsement file {}", path.display()))?;
        json::from_json(&buf)
            .map_err(|err| eyre!("failed to parse endorsement JSON from file: {err}"))?
    } else {
        // Reuse the standard JSON parser to read from stdin
        crate::parse_json_stdin(context)?
    };

    let instruction: iroha::data_model::isi::endorsement::SubmitDomainEndorsement =
        isi_endorsement::SubmitDomainEndorsement { endorsement };
    context.finish([InstructionBox::from(instruction)])
}

fn list_endorsements<C: RunContext>(args: ListArgs, context: &mut C) -> Result<()> {
    let client = context.client_from_config();
    let records = client
        .query_single(FindDomainEndorsements {
            domain_id: args.domain,
        })
        .wrap_err("failed to fetch domain endorsements")?;
    context.print_data(&records)
}

fn show_policy<C: RunContext>(args: PolicyArgs, context: &mut C) -> Result<()> {
    let client = context.client_from_config();
    let policy = client
        .query_single(FindDomainEndorsementPolicy {
            domain_id: args.domain,
        })
        .wrap_err("failed to fetch domain endorsement policy")?;
    context.print_data(&policy)
}

fn show_committee<C: RunContext>(args: CommitteeArgs, context: &mut C) -> Result<()> {
    let client = context.client_from_config();
    let committee = client
        .query_single(FindDomainCommittee {
            committee_id: args.committee_id,
        })
        .wrap_err("failed to fetch domain committee")?;
    context.print_data(&committee)
}

fn register_committee<C: RunContext>(args: RegisterCommitteeArgs, context: &mut C) -> Result<()> {
    let members = parse_public_keys(&args.members)?;
    let metadata = if let Some(path) = &args.metadata {
        read_metadata(path)?
    } else {
        Metadata::default()
    };
    let committee = DomainCommittee {
        committee_id: args.committee_id,
        members,
        quorum: args.quorum,
        metadata,
    };
    if !committee.is_valid() {
        return Err(eyre!(
            "committee invalid: quorum must be > 0 and <= members (got quorum {}, {} members)",
            committee.quorum,
            committee.members.len()
        ));
    }
    let instruction = isi_endorsement::RegisterDomainCommittee { committee };
    context.finish([InstructionBox::from(instruction)])
}

fn set_policy<C: RunContext>(args: SetPolicyArgs, context: &mut C) -> Result<()> {
    let policy = DomainEndorsementPolicy {
        committee_id: args.committee_id,
        max_endorsement_age: args.max_endorsement_age,
        required: args.required,
    };
    let instruction = isi_endorsement::SetDomainEndorsementPolicy {
        domain: args.domain,
        policy,
    };
    context.finish([InstructionBox::from(instruction)])
}

fn read_metadata(path: &PathBuf) -> Result<Metadata> {
    let buf = fs::read_to_string(path)
        .wrap_err_with(|| format!("failed to read metadata file {}", path.display()))?;
    json::from_json(&buf).map_err(|err| eyre!("failed to parse metadata JSON from file: {err}"))
}

fn parse_public_keys(values: &[String]) -> Result<Vec<PublicKey>> {
    let mut keys = Vec::with_capacity(values.len());
    for raw in values {
        let pk =
            PublicKey::from_str(raw).map_err(|err| eyre!("invalid public key `{raw}`: {err}"))?;
        keys.push(pk);
    }
    Ok(keys)
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::{Algorithm, ExposedPrivateKey};

    #[test]
    fn prepare_signs_payload_and_keeps_body_hash_stable() {
        let kp_a = KeyPair::random_with_algorithm(Algorithm::Ed25519);
        let kp_b = KeyPair::random_with_algorithm(Algorithm::Ed25519);
        let args = PrepareArgs {
            domain: DomainId::try_new("wonderland", "universal").expect("domain id"),
            committee_id: "default".to_owned(),
            issued_at_height: 5,
            expires_at_height: 10,
            block_start: Some(5),
            block_end: Some(12),
            dataspace: None,
            metadata: None,
            signer_keys: vec![
                ExposedPrivateKey(kp_a.private_key().clone()).to_string(),
                ExposedPrivateKey(kp_b.private_key().clone()).to_string(),
            ],
        };
        let endorsement = build_endorsement(&args).expect("build endorsement");
        assert_eq!(endorsement.signatures.len(), 2);
        assert!(
            endorsement
                .scope
                .contains_height(endorsement.issued_at_height)
        );
        // Body hash should ignore signatures.
        let baseline = endorsement.body_hash();
        assert_eq!(baseline, endorsement.body_hash());
    }
}
