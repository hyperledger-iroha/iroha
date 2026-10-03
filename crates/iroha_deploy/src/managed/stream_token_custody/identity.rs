//! Original generated authority and signed-genesis identity; no response-selected trust roots.

use super::*;
use iroha_crypto::{Algorithm, ExposedPrivateKey, KeyPair};
use iroha_data_model::{
    NetworkId,
    account::address::ChainDiscriminantGuard,
    sumeragi_finality::{FinalityValidator, genesis_epoch},
};
use sorafs_manifest::signer::protocol::{SignerPurposeBindingV1, SignerRoleV1};
use std::collections::BTreeSet;
use zeroize::Zeroizing;

pub(super) fn open(prepared: &PreparedLocalnet) -> Result<ManagedStreamTokenCustody> {
    let manifest = prepared.stream_token_authorities()?.ok_or_else(|| {
        invalid("managed custody requires its original StreamTokenAuthorities profile")
    })?;
    let config = prepared.context.load_client_config()?;
    let _profile = ChainDiscriminantGuard::enter(config.account_chain_discriminant);
    let path = prepared
        .context
        .client_config
        .parent()
        .ok_or_else(|| invalid("managed custody has no generation"))?;
    let generation = PrivateDirectory::open_exact(path)?;
    let bytes = generation.read(
        "genesis.signed.nrt",
        iroha_genesis::SIGNED_GENESIS_MAX_BYTES_V1,
    )?;
    let genesis = iroha_data_model::block::decode_framed_signed_block(&bytes)
        .map_err(|_| invalid("invalid original signed custody genesis"))?;
    let epoch = genesis_epoch(&genesis)
        .map_err(|_| invalid("cannot authenticate original custody genesis"))?;
    if epoch.network_id != config.network_id
        || NetworkId::from_genesis_hash(genesis.hash()) != manifest.network_id
        || prepared.context.dataspace_id != 0
        || manifest.manager != config.account
    {
        return Err(invalid("original custody network or manager differs"));
    }
    let validators = epoch
        .committee
        .iter()
        .map(|member| FinalityValidator {
            public_key: member.validator.public_key().clone(),
            proof_of_possession: member.proof_of_possession.clone(),
        })
        .collect();
    let expected: BTreeSet<_> = epoch
        .committee
        .iter()
        .map(|member| member.validator.clone())
        .collect();
    let mut peers = Vec::new();
    for peer in &prepared.peers {
        let bytes = iroha_fs::read_private(&peer.config_path, 1024 * 1024)?;
        let rendered = std::str::from_utf8(&bytes)
            .map_err(|_| invalid("invalid retained custody peer configuration"))?;
        let table = crate::secret_toml::parse_table(rendered, "managed custody peer")
            .map_err(|_| invalid("invalid retained custody peer configuration"))?;
        let reader = iroha_config::node_config::open_node_config(
            iroha_config::node_config::NodeFile::Verified {
                path: peer.config_path.clone(),
                table,
            },
            iroha_config::node_config::NodeConfigOptions::default(),
        )
        .map_err(|_| invalid("cannot resolve retained custody peer"))?;
        let (user, _) = reader
            .read()
            .map_err(|_| invalid("cannot read retained custody peer"))?;
        let actual = user
            .parse()
            .map_err(|_| invalid("invalid retained custody peer"))?;
        let id = PeerId::new(actual.common.key_pair.public_key().clone());
        if actual.genesis.expected_hash != genesis.hash() || !expected.contains(&id) {
            return Err(invalid(
                "custody peer differs from original genesis committee",
            ));
        }
        let mut selected = config.clone();
        selected.torii_api_url = peer
            .torii_url
            .parse()
            .map_err(|_| invalid("invalid custody peer endpoint"))?;
        let client = Client::builder(selected)
            .build()
            .map_err(|_| invalid("cannot construct custody peer client"))?;
        peers.push((id, client));
    }
    if peers.len() != expected.len()
        || peers
            .iter()
            .map(|(peer, _)| peer.clone())
            .collect::<BTreeSet<_>>()
            != expected
    {
        return Err(invalid(
            "custody endpoints do not cover the exact original committee",
        ));
    }
    let directory = generation
        .ensure_child("runtime")?
        .ensure_child("stream-token-custody")?;
    let lock = directory.open_lock("operation.lock")?;
    lock.try_lock()
        .map_err(|_| invalid("another managed custody operation holds this generation"))?;
    directory.revalidate()?;
    Ok(ManagedStreamTokenCustody {
        prepared: prepared.clone(),
        directory,
        _lock: lock,
        manifest,
        config: config.clone(),
        genesis: GenesisAnchor {
            network_id: config.network_id,
            chain_id: config.chain.to_string(),
            genesis,
            validators,
        },
        peers,
    })
}

impl ManagedStreamTokenCustody {
    pub(super) fn role(&self, role: StreamTokenAuthorityRole) -> Result<&AccountId> {
        self.manifest
            .authorities
            .iter()
            .find(|authority| authority.role == role)
            .map(|authority| &authority.account)
            .ok_or_else(|| invalid("original custody role is absent"))
    }
    pub(super) fn validate_profile(&self) -> Result<()> {
        self.directory.revalidate()?;
        if iroha_fs::FileIdentity::of(&self.directory.open_read("operation.lock")?)?
            != iroha_fs::FileIdentity::of(&self._lock)?
        {
            return Err(invalid("managed custody operation lock was replaced"));
        }
        if self.prepared.stream_token_authorities()?.as_ref() != Some(&self.manifest) {
            return Err(invalid("original custody authority profile changed"));
        }
        Ok(())
    }
    pub(super) fn validate_policy(&self, policy: &SignerCustodyPolicyV1) -> Result<()> {
        journal::encode(policy, 16 * 1024)?;
        policy
            .validate()
            .map_err(|_| invalid("invalid managed custody policy"))?;
        if policy.binding.chain_id != self.config.chain.to_string()
            || policy.binding.network_id != *self.config.network_id.as_bytes()
            || policy.binding.role != SignerRoleV1::StreamToken
            || policy.binding.purpose
                != (SignerPurposeBindingV1::StreamToken {
                    provider_id: *self.manifest.provider_id.as_bytes(),
                })
            || self
                .role(StreamTokenAuthorityRole::TokenSigner)?
                .try_signatory()
                != Some(&policy.binding.public_key)
            || self
                .role(StreamTokenAuthorityRole::CustodyAttester)?
                .try_signatory()
                != Some(&policy.attester_public_key)
            || policy.binding.public_key == *self.config.key_pair.public_key()
            || policy.attester_public_key == *self.config.key_pair.public_key()
            || policy.binding.key_revision != 1
            || policy.binding.policy_revision != 1
            || policy.attester_authority.key_revision != 1
            || policy.attester_authority.policy_revision != 1
        {
            return Err(invalid(
                "initial custody policy differs from independent generated roles",
            ));
        }
        Ok(())
    }
    pub(super) fn validate_original(&self, original: &Original) -> Result<()> {
        original.validate()?;
        let verifier = self.decode_checkpoint(&original.checkpoint)?;
        verifier
            .verified_tip()
            .map_err(|_| invalid("invalid original custody checkpoint"))?
            .verify_global_scope(self.config.network_id, &self.config.chain.to_string())
            .map_err(|_| invalid("original custody checkpoint is not the selected Global root"))?;
        if original.selection.provider_id != self.manifest.provider_id {
            return Err(invalid("original custody provider differs"));
        }
        match &original.action {
            Action::Configure(policy) => {
                self.validate_policy(policy)?;
                if original.selection.current.is_some()
                    || original.selection.expected_revision != 0
                    || original.selection.expected_digest != [0; 32]
                    || original.selection.binding != policy.binding
                {
                    return Err(invalid(
                        "initial custody configuration has a substituted predecessor",
                    ));
                }
            }
            Action::Enroll {
                enrollment, anchor, ..
            } => {
                let current = original
                    .selection
                    .current
                    .as_ref()
                    .ok_or_else(|| invalid("enrollment lacks configured predecessor"))?;
                let control: sorafs_manifest::signer::custody_control::SignerCustodyControlStateV1 =
                    norito::decode_canonical_with_limits(
                        &current.control_state,
                        norito::DecodeLimits::new(4096, 16 * 1024, 16 * 1024, 1024 * 1024, 32),
                    )
                    .map_err(|_| invalid("invalid original governed custody control"))?;
                self.validate_policy(&control.policy)?;
                if control.policy.binding != original.selection.binding
                    || current.revision != 1
                    || original.selection.expected_revision != current.revision
                    || current
                        .canonical_digest()
                        .map_err(|_| invalid("invalid original custody digest"))?
                        != original.selection.expected_digest
                    || anchor.state_digest != original.selection.expected_digest
                    || anchor.height != verifier.checkpoint().height()
                    || anchor.block_hash
                        != *verifier.checkpoint().tip().block_header.hash().as_ref()
                    || enrollment.len() > 16 * 1024
                {
                    return Err(invalid(
                        "original enrollment differs from its exact native anchor",
                    ));
                }
                let statement: SignerCustodyRecordV1 = norito::decode_canonical_with_limits(
                    enrollment,
                    norito::DecodeLimits::new(4096, 16 * 1024, 16 * 1024, 1024 * 1024, 32),
                )
                .map_err(|_| invalid("invalid retained enrollment statement"))?;
                if statement.statement.evidence_digest
                    != self.evidence_digest(
                        &control.policy,
                        &original.selection,
                        &original.checkpoint,
                    )?
                {
                    return Err(invalid(
                        "original enrollment evidence differs from authenticated inputs",
                    ));
                }
            }
        }
        Ok(())
    }
    pub(super) fn selection(
        &self,
        binding: &SignerCustodyBindingV1,
        state: &VerifiedStreamTokenCustodyStateV1,
    ) -> Result<StreamTokenCustodySelection> {
        if state.network_id() != self.config.network_id
            || state.provider_id() != self.manifest.provider_id
            || state.owner() != self.role(StreamTokenAuthorityRole::IssuerOperator)?
        {
            return Err(invalid(
                "verified custody state differs from independently selected owner",
            ));
        }
        Ok(StreamTokenCustodySelection {
            provider_id: self.manifest.provider_id,
            binding: binding.clone(),
            expected_revision: state
                .current()
                .map_or(0, |current| current.record().revision),
            expected_digest: state
                .current()
                .map_or([0; 32], |current| current.anchor().state_digest),
            current: state.current().map(|current| current.record().clone()),
        })
    }
    pub(super) fn attester(&self) -> Result<KeyPair> {
        let path = self
            .prepared
            .context
            .client_config
            .parent()
            .ok_or_else(|| invalid("missing custody generation"))?
            .join("runtime")
            .join("stream-token-authorities");
        let authority = PrivateDirectory::open_exact(&path)?;
        let bytes = authority.read(
            StreamTokenAuthorityRole::CustodyAttester.credential_filename(),
            256,
        )?;
        let text = bytes
            .strip_suffix(b"\n")
            .and_then(|bytes| std::str::from_utf8(bytes).ok())
            .ok_or_else(|| invalid("invalid original attester credential"))?;
        let text = Zeroizing::new(text.to_owned());
        let exposed: ExposedPrivateKey = text
            .parse()
            .map_err(|_| invalid("invalid original attester credential"))?;
        let key = KeyPair::from_private_key(exposed.0)
            .map_err(|_| invalid("invalid original attester key"))?;
        if key.public_key().algorithm() != Algorithm::Ed25519
            || self
                .role(StreamTokenAuthorityRole::CustodyAttester)?
                .try_signatory()
                != Some(key.public_key())
        {
            return Err(invalid("original independent attester key changed"));
        }
        Ok(key)
    }
    pub(super) fn evidence_digest(
        &self,
        policy: &SignerCustodyPolicyV1,
        selection: &StreamTokenCustodySelection,
        checkpoint: &[u8],
    ) -> Result<[u8; 32]> {
        let _profile = ChainDiscriminantGuard::enter(self.config.account_chain_discriminant);
        let profile = norito::json::to_vec(&self.manifest)
            .map_err(|_| invalid("cannot encode original authority profile"))?;
        let policy = journal::encode(policy, 16 * 1024)?;
        let selection = journal::encode(selection, 64 * 1024)?;
        Ok(*Hash::new_from_chunks(&[
            b"iroha:managed-stream-token-custody-evidence:v1\0",
            self.genesis.genesis.hash().as_ref(),
            &profile,
            &policy,
            &selection,
            Hash::new(checkpoint).as_ref(),
        ])
        .as_ref())
    }
}
