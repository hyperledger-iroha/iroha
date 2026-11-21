//! Offline review, signatures and assembly of exact child-administrator certificates.
//!
//! A supplied State file is review input, not proof of authority. Only the founded child's
//! committed State authorizes an action when its sole original signed instruction executes.

use std::path::PathBuf;

use eyre::{Result, eyre};
use iroha::data_model::{
    isi::{InstructionBox, child_runtime::AuthorizeChildRuntimeActionV1},
    runtime::{
        child_admin::{
            ChildAdminSignatureV1, ChildRuntimeAdminActionV1, ChildRuntimeAdminCertificateV1,
            ChildRuntimeAdminPlanV1, ChildRuntimeAdminPolicyV1, ChildRuntimeAdminStateV1,
            MAX_CHILD_ADMIN_SIGNERS_V1,
        },
        lifecycle::{ChangeSetEndorsementV1, ChangeSetId, ChangeSetV1},
    },
};

use super::{decode_hex_32, read_json, read_private_key};
use crate::{Run, RunContext};

/// Prepare, sign, assemble and relay the founded child's administrator approval.
#[derive(clap::Subcommand, Debug)]
pub enum ChildAdminCommand {
    /// Print an exact action plan from explicit review inputs; does not contact a node
    Prepare(PrepareArgs),
    /// Sign the entire plan with an explicit current administrator key; does not submit
    Sign(SignArgs),
    /// Verify distinct signatures against the reviewed policy and print the certificate
    Assemble(AssembleArgs),
    /// Relay one certificate using the ordinary configured payer, or emit its sole instruction
    Submit(SubmitArgs),
}

impl ChildAdminCommand {
    /// Only commands that cannot sign an outer transaction or contact a node need no config.
    pub(super) fn allows_fallback_config(&self) -> bool {
        !matches!(self, Self::Submit(_))
    }
}

impl Run for ChildAdminCommand {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        match self {
            Self::Prepare(args) => args.run(context),
            Self::Sign(args) => args.run(context),
            Self::Assemble(args) => args.run(context),
            Self::Submit(args) => args.run(context),
        }
    }
}

/// Explicit review context; a local file never establishes genesis or current State authority.
#[derive(clap::Args, Debug)]
pub struct StateArgs {
    /// JSON ChildRuntimeAdminStateV1 independently checked against the child's committed State;
    /// the CLI checks consistency only and does not authenticate this file
    #[arg(long, value_name = "PATH")]
    state: PathBuf,
}

impl StateArgs {
    fn read(&self) -> Result<ChildRuntimeAdminStateV1> {
        let state: ChildRuntimeAdminStateV1 = read_json(&self.state, "reviewed child admin state")?;
        state.validate()?;
        Ok(state)
    }
}

/// Exact plan preparation, with no inferred administrator, nonce or expiry.
#[derive(clap::Args, Debug)]
pub struct PrepareArgs {
    #[command(flatten)]
    state: StateArgs,
    /// First certified execution height, inclusive
    #[arg(long, value_name = "HEIGHT")]
    valid_from: u64,
    /// Expiry certified height, exclusive
    #[arg(long, value_name = "HEIGHT")]
    valid_until: u64,
    #[command(subcommand)]
    action: PrepareAction,
}

/// One complete intended administrative effect.
#[derive(clap::Subcommand, Debug)]
pub enum PrepareAction {
    /// Schedule an exact child-scoped change set; validator support remains independent
    Schedule {
        /// JSON ChangeSetV1 from the child's runtime draft command
        #[arg(long, value_name = "PATH")]
        change_set: PathBuf,
        /// JSON ChangeSetEndorsementV1; repeat for each required release signer
        #[arg(long = "endorsement", value_name = "PATH")]
        endorsements: Vec<PathBuf>,
    },
    /// Cancel one scheduled change on this child
    Cancel {
        /// Exact change-set identity, 32-byte hex
        #[arg(long, value_name = "HEX")]
        id: String,
    },
    /// Rotate the entire policy to exactly the next revision, keeping the replay cursor
    Rotate {
        /// JSON ChildRuntimeAdminPolicyV1 with the explicit next roster, threshold and notice
        #[arg(long, value_name = "PATH")]
        policy: PathBuf,
    },
}

impl Run for PrepareArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let state = self.state.read()?;
        let action = match self.action {
            PrepareAction::Schedule {
                change_set,
                endorsements,
            } => {
                if endorsements.len() > MAX_CHILD_ADMIN_SIGNERS_V1 {
                    return Err(eyre!("too many release endorsements"));
                }
                ChildRuntimeAdminActionV1::Schedule {
                    change_set: read_json::<ChangeSetV1>(&change_set, "change set")?,
                    provenance: endorsements
                        .iter()
                        .map(|path| read_json::<ChangeSetEndorsementV1>(path, "endorsement"))
                        .collect::<Result<Vec<_>>>()?,
                }
            }
            PrepareAction::Cancel { id } => {
                ChildRuntimeAdminActionV1::Cancel(ChangeSetId(decode_hex_32("change-set id", &id)?))
            }
            PrepareAction::Rotate { policy } => ChildRuntimeAdminActionV1::RotatePolicy(
                read_json::<ChildRuntimeAdminPolicyV1>(&policy, "next administrator policy")?,
            ),
        };
        let plan = ChildRuntimeAdminPlanV1 {
            root: state.root,
            policy_id: state.policy.id()?,
            policy_revision: state.policy.revision,
            nonce: state.next_nonce,
            valid_from: self.valid_from,
            valid_until: self.valid_until,
            action,
        };
        check_plan(&state, &plan)?;
        context.print_data(&plan)
    }
}

/// Sign exactly the reviewed plan, without producing an outer transaction.
#[derive(clap::Args, Debug)]
pub struct SignArgs {
    #[command(flatten)]
    state: StateArgs,
    /// JSON ChildRuntimeAdminPlanV1 to sign without changing any field
    #[arg(long, value_name = "PATH")]
    plan: PathBuf,
    /// Runtime-only private key file (multihash hex); its contents are never printed
    #[arg(long, value_name = "PATH")]
    admin_key_file: PathBuf,
}

impl Run for SignArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let state = self.state.read()?;
        let plan: ChildRuntimeAdminPlanV1 = read_json(&self.plan, "child administrator plan")?;
        check_plan(&state, &plan)?;
        let key = iroha::crypto::KeyPair::from(read_private_key(&self.admin_key_file)?);
        if state.policy.admins.binary_search(key.public_key()).is_err() {
            return Err(eyre!(
                "the signing key is not in the reviewed administrator policy"
            ));
        }
        context.print_data(&ChildAdminSignatureV1::sign(&plan, &key)?)
    }
}

/// Assemble signatures only after every signature and the reviewed threshold verify.
#[derive(clap::Args, Debug)]
pub struct AssembleArgs {
    #[command(flatten)]
    state: StateArgs,
    /// JSON ChildRuntimeAdminPlanV1 the administrators signed
    #[arg(long, value_name = "PATH")]
    plan: PathBuf,
    /// JSON ChildAdminSignatureV1; repeat for distinct current administrators
    #[arg(long = "signature", value_name = "PATH", required = true)]
    signatures: Vec<PathBuf>,
    /// Explicit intended certified execution height used for local expiry checks
    #[arg(long, value_name = "HEIGHT")]
    at_height: u64,
}

impl Run for AssembleArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let state = self.state.read()?;
        let plan = read_json(&self.plan, "child administrator plan")?;
        if self.signatures.len() > MAX_CHILD_ADMIN_SIGNERS_V1 {
            return Err(eyre!("too many administrator signatures"));
        }
        let mut signatures: Vec<ChildAdminSignatureV1> = self
            .signatures
            .iter()
            .map(|path| read_json(path, "administrator signature"))
            .collect::<Result<_>>()?;
        signatures.sort_by(|left, right| left.signer.cmp(&right.signer));
        let certificate = ChildRuntimeAdminCertificateV1 { plan, signatures };
        check_plan(&state, &certificate.plan)?;
        certificate.verify_against(&state, self.at_height)?;
        context.print_data(&certificate)
    }
}

/// Relay a complete certificate, preserving the sole original native-instruction contract.
#[derive(clap::Args, Debug)]
pub struct SubmitArgs {
    #[command(flatten)]
    state: StateArgs,
    /// JSON ChildRuntimeAdminCertificateV1; no plan or signature field is rewritten
    #[arg(long, value_name = "PATH")]
    certificate: PathBuf,
    /// Intended certified execution height for local checks; Core checks the actual height
    #[arg(long, value_name = "HEIGHT")]
    at_height: u64,
}

impl Run for SubmitArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        if context.input_instructions() {
            return Err(eyre!(
                "child administration requires one instruction; --stdin-instructions is forbidden"
            ));
        }
        if !context.has_client_config() {
            return Err(eyre!(
                "child certificate submission requires an explicit client configuration"
            ));
        }
        let state = self.state.read()?;
        let certificate: ChildRuntimeAdminCertificateV1 =
            read_json(&self.certificate, "child administrator certificate")?;
        check_plan(&state, &certificate.plan)?;
        certificate.verify_against(&state, self.at_height)?;
        let config = context.config();
        if config.network_id != certificate.plan.root.network {
            return Err(eyre!(
                "the configured network differs from the certified child network"
            ));
        }
        let instance = certificate.plan.root.scope().instance_id(
            &iroha_core::sumeragi::crypto::BlsCrypto::new(),
            config.network_id,
            config.chain.as_str(),
        )?;
        if instance.0 != certificate.plan.root.instance {
            return Err(eyre!(
                "the configured chain and reviewed scope differ from the certified native child instance"
            ));
        }
        if config.key_pair.public_key().algorithm() != iroha::crypto::Algorithm::Ed25519 {
            return Err(eyre!(
                "the sole native child instruction requires an Ed25519 outer transaction signer"
            ));
        }
        context.finish(vec![InstructionBox::from(AuthorizeChildRuntimeActionV1 {
            certificate,
        })])
    }
}

/// Check exact review context before signing; this deliberately confers no live authority.
fn check_plan(state: &ChildRuntimeAdminStateV1, plan: &ChildRuntimeAdminPlanV1) -> Result<()> {
    state.validate()?;
    plan.validate()?;
    if plan.root != state.root
        || plan.policy_id != state.policy.id()?
        || plan.policy_revision != state.policy.revision
        || plan.nonce != state.next_nonce
        || state.next_nonce == u64::MAX
    {
        return Err(eyre!(
            "the plan does not match the complete reviewed root, policy and next nonce"
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests;
