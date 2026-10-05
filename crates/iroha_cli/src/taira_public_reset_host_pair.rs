//! Canonical MacStadium host custody and authenticated cross-host phase checkpoints.
//!
//! A phase checkpoint is signed by the operator key admitted by the reset inventory.
//! It names both physical hosts and their custody roots. Transport authentication
//! cannot replace this signature, and a checkpoint cannot authorize another reset.

use super::*;
use iroha_crypto::{KeyPair, Signature};

pub(super) const NATIVE_EDGE_TARGET: &str = "aarch64-apple-darwin";
const HOST_PAIR_SCHEMA: &str = "iroha.taira.public-reset.host-pair.v1";
const CHECKPOINT_SCHEMA: &str = "iroha.taira.public-reset.host-phase-checkpoint.v1";
const CHECKPOINT_DOMAIN: &[u8] = b"iroha:taira:public-reset:host-phase-checkpoint:v1\0";
pub(super) const MAX_CHECKPOINT_BYTES: usize = 16 * 1024;

/// Verify every signature and predecessor, rather than trusting the final member alone.
pub(super) fn verify_checkpoint_chain(
    checkpoints: &[SignedHostPhaseV1],
    inventory: &InventoryV1,
    inventory_sha256: &str,
    authorization_sha256: &str,
    execution_expires_at_unix_ms: u64,
    required_phase: HostPhaseV1,
) -> Result<()> {
    if checkpoints.len() != usize::from(required_phase.sequence()) {
        return Err(eyre!(
            "cross-host phase requires its complete exact checkpoint chain"
        ));
    }
    for (index, checkpoint) in checkpoints.iter().enumerate() {
        checkpoint.verify(
            inventory,
            inventory_sha256,
            authorization_sha256,
            execution_expires_at_unix_ms,
            index.checked_sub(1).map(|previous| &checkpoints[previous]),
        )?;
    }
    if checkpoints.last().map(|checkpoint| checkpoint.claims.phase) != Some(required_phase) {
        return Err(eyre!("cross-host checkpoint chain ends at another phase"));
    }
    Ok(())
}

/// Embedded native owner helpers are identical in the guest and Darwin CLI artifacts.
pub(super) fn helper_source_closure_sha256() -> String {
    let mut digest = Sha256::new();
    digest.update(b"iroha:taira:public-reset:native-helper-source:v1\0");
    for (name, source) in [
        (
            "taira_native_nginx_check.py",
            include_bytes!("../../../scripts/taira_native_nginx_check.py").as_slice(),
        ),
        (
            "taira_native_nginx_apply.py",
            include_bytes!("../../../scripts/taira_native_nginx_apply.py").as_slice(),
        ),
        (
            "taira_native_validator_forwarding.py",
            include_bytes!("../../../scripts/taira_native_validator_forwarding.py").as_slice(),
        ),
        (
            "taira_native_edge_completion.py",
            include_bytes!("../../../scripts/taira_native_edge_completion.py").as_slice(),
        ),
    ] {
        digest.update(name.as_bytes());
        digest.update([0]);
        digest.update(
            u64::try_from(source.len())
                .expect("embedded source length fits u64")
                .to_be_bytes(),
        );
        digest.update(source);
    }
    hex::encode(digest.finalize())
}

/// One physical host's independently selected SSH identity and native custody.
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub(super) struct ResetHostV1 {
    pub(super) endpoint: HostSshRouteV1,
    pub(super) platform: PlatformV1,
    pub(super) owner_uid: u32,
    pub(super) owner_gid: u32,
    pub(super) owner_home: String,
    pub(super) custody_root: String,
    /// Independently installed same-platform dispatcher; candidate releases do not replace it.
    pub(super) dispatcher_path: String,
    pub(super) dispatcher_sha256: String,
    pub(super) guard_sha256: String,
    /// Native software custody key; its private material never leaves this physical host.
    pub(super) capture_public_key: String,
}

/// Physical SSH route only; role upload guards and release programs are separate owners.
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub(super) struct HostSshRouteV1 {
    pub(super) hostname: String,
    pub(super) port: u16,
    pub(super) user: String,
    pub(super) known_host_line_sha256: String,
    pub(super) host_identity_sha256: String,
}

/// The only public-reset transport topology: a Linux guest and a native Mac edge.
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub(super) struct ResetHostPairV1 {
    pub(super) schema: String,
    pub(super) provider: String,
    pub(super) validator_guest: ResetHostV1,
    pub(super) native_edge: ResetHostV1,
}

impl ResetHostPairV1 {
    pub(super) fn validate(&self) -> Result<()> {
        if self.schema != HOST_PAIR_SCHEMA || self.provider != "macstadium-dublin" {
            return Err(eyre!(
                "public-reset requires the canonical MacStadium Dublin host pair"
            ));
        }
        self.validator_guest.validate(false)?;
        self.native_edge.validate(true)?;
        if self.validator_guest.endpoint.host_identity_sha256
            == self.native_edge.endpoint.host_identity_sha256
            || self.validator_guest.endpoint.hostname == self.native_edge.endpoint.hostname
        {
            return Err(eyre!(
                "validator guest and native edge must have distinct authenticated physical hosts"
            ));
        }
        Ok(())
    }

    pub(super) fn digest(&self) -> Result<String> {
        self.validate()?;
        Ok(sha256_hex(json::to_json(self)?.as_bytes()))
    }

    /// Compare the immutable physical owners before admitting separately proven native upgrades.
    /// Dispatcher/guard digests are intentionally excluded here; callers must validate their
    /// successor through native captured custody and the signed candidate/transition receipt.
    pub(super) fn validate_physical_binding(&self, predecessor: &Self) -> Result<()> {
        self.validate()?;
        predecessor.validate()?;
        if self.provider != predecessor.provider {
            return Err(eyre!(
                "host successor changes the independently approved provider"
            ));
        }
        for (current, previous) in [
            (&self.validator_guest, &predecessor.validator_guest),
            (&self.native_edge, &predecessor.native_edge),
        ] {
            if json::to_json(&current.endpoint)? != json::to_json(&previous.endpoint)?
                || json::to_json(&current.platform)? != json::to_json(&previous.platform)?
                || current.owner_uid != previous.owner_uid
                || current.owner_gid != previous.owner_gid
                || current.owner_home != previous.owner_home
                || current.custody_root != previous.custody_root
                || current.dispatcher_path != previous.dispatcher_path
                || current.capture_public_key != previous.capture_public_key
            {
                return Err(eyre!(
                    "host successor changes the immutable physical route, platform or software custodian"
                ));
            }
        }
        Ok(())
    }

    pub(super) fn validate_roles(&self, validators: &[ValidatorV1], edge: &EdgeV1) -> Result<()> {
        self.validate()?;
        if validators.len() != VALIDATOR_SLUGS.len() {
            return Err(eyre!("host pair requires the exact four validator roles"));
        }
        if validators[0].endpoint.upload_guard_sha256 != self.validator_guest.guard_sha256
            || edge.endpoint.upload_guard_sha256 != self.native_edge.guard_sha256
        {
            return Err(eyre!(
                "host custodian guard differs from its admitted coordinator role guard"
            ));
        }
        for (validator, slug) in validators.iter().zip(VALIDATOR_SLUGS) {
            if validator.slug != slug {
                return Err(eyre!(
                    "validator role order differs from the canonical host pair"
                ));
            }
            self.validator_guest.bind_endpoint(&validator.endpoint)?;
            if validator.platform.os != self.validator_guest.platform.os
                || validator.platform.arch != self.validator_guest.platform.arch
                || validator.platform.kvm_api_version
                    != self.validator_guest.platform.kvm_api_version
            {
                return Err(eyre!(
                    "validator platform differs from its admitted Linux guest"
                ));
            }
        }
        self.native_edge.bind_endpoint(&edge.endpoint)?;
        if edge.platform.os != self.native_edge.platform.os
            || edge.platform.arch != self.native_edge.platform.arch
            || edge.platform.kvm_api_version != 0
            || edge.reset_guard != format!("{}/taira-edge", self.native_edge.custody_root)
        {
            return Err(eyre!(
                "native edge platform or guard differs from its admitted Mac custodian"
            ));
        }
        Ok(())
    }
}

impl ResetHostV1 {
    fn validate(&self, native_edge: bool) -> Result<()> {
        let capture_key = PublicKey::from_str(&self.capture_public_key)?;
        if capture_key.try_algorithm()? != Algorithm::Ed25519 {
            return Err(eyre!(
                "host capture custody requires an independently admitted Ed25519 key"
            ));
        }
        validate_hostname(&self.endpoint.hostname)?;
        for (label, value) in [
            (
                "host known-host line",
                &self.endpoint.known_host_line_sha256,
            ),
            (
                "physical host identity",
                &self.endpoint.host_identity_sha256,
            ),
            ("native host guard", &self.guard_sha256),
            ("native dispatcher", &self.dispatcher_sha256),
        ] {
            validate_lower_hex(label, value, 64)?;
        }
        if self.endpoint.port != 22
            || self.endpoint.user.is_empty()
            || !self
                .endpoint
                .user
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'_' | b'-'))
        {
            return Err(eyre!(
                "host pair requires an exact named SSH user at port 22"
            ));
        }
        for path in [&self.owner_home, &self.custody_root, &self.dispatcher_path] {
            validate_absolute_normal_path(Path::new(path), "host custody path")?;
            if path.bytes().any(|c| c.is_ascii_whitespace()) {
                return Err(eyre!(
                    "native custody paths must be canonical without whitespace"
                ));
            }
        }
        if native_edge {
            if self.platform.os != "macos"
                || self.platform.arch != "aarch64"
                || self.platform.kvm_api_version != 0
                || self.owner_uid == 0
                || self.endpoint.user == "root"
                || self.owner_home != format!("/Users/{}", self.endpoint.user)
                || self.custody_root
                    != format!(
                        "{}/.local/share/iroha/taira/public-reset-v1",
                        self.owner_home
                    )
                || self.dispatcher_path != format!("{}/dispatcher/iroha", self.custody_root)
            {
                return Err(eyre!(
                    "native edge requires a named Mac owner, Darwin/AArch64 and exact private custody roots"
                ));
            }
        } else if self.platform.os != "linux"
            || self.platform.arch != "aarch64"
            || !matches!(self.platform.kvm_api_version, 0 | 12)
            || self.owner_uid != 0
            || self.owner_gid != 0
            || self.endpoint.user != "root"
            || self.owner_home != "/root"
            || self.custody_root != "/var/lib/taira/.public-reset-control-v1"
            || self.dispatcher_path != "/usr/local/libexec/iroha-taira-public-reset-v1"
        {
            return Err(eyre!(
                "validators require the root-owned Linux guest and exact dispatcher custody"
            ));
        }
        Ok(())
    }

    fn bind_endpoint(&self, endpoint: &EndpointV1) -> Result<()> {
        if endpoint.hostname != self.endpoint.hostname
            || endpoint.port != self.endpoint.port
            || endpoint.user != self.endpoint.user
            || endpoint.known_host_line_sha256 != self.endpoint.known_host_line_sha256
            || endpoint.host_identity_sha256 != self.endpoint.host_identity_sha256
        {
            return Err(eyre!(
                "role SSH route differs from its independently admitted physical host"
            ));
        }
        // Upload guards are role-specific; the host guard is a separate custodian.
        Ok(())
    }
}

/// Ordered cross-host decisions. Local effects remain owned by each native journal.
#[derive(Clone, Copy, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(tag = "kind", content = "value", deny_unknown_fields)]
pub(super) enum HostPhaseV1 {
    #[norito(rename = "candidate_frontier")]
    CandidateFrontier,
    #[norito(rename = "native_edge_ready")]
    NativeEdgeReady,
    #[norito(rename = "deployment_proven")]
    DeploymentProven,
}

impl HostPhaseV1 {
    pub(super) const fn sequence(self) -> u8 {
        match self {
            Self::CandidateFrontier => 1,
            Self::NativeEdgeReady => 2,
            Self::DeploymentProven => 3,
        }
    }
}

/// Bounded immutable evidence enabling exactly one cross-host transition.
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub(super) struct HostPhaseClaimsV1 {
    pub(super) schema: String,
    pub(super) phase: HostPhaseV1,
    pub(super) deployment_id: String,
    pub(super) inventory_sha256: String,
    pub(super) authorization_sha256: String,
    pub(super) authorization_nonce: String,
    pub(super) host_pair_sha256: String,
    pub(super) guest_host_identity_sha256: String,
    pub(super) native_edge_host_identity_sha256: String,
    pub(super) guest_custody_root: String,
    pub(super) native_edge_custody_root: String,
    pub(super) source_commit: String,
    pub(super) next_genesis_hash: String,
    pub(super) evidence_sha256: String,
    /// Null only for the first checkpoint. Subsequent phases bind the exact predecessor.
    #[norito(required)]
    pub(super) predecessor_sha256: Option<String>,
    pub(super) execution_expires_at_unix_ms: u64,
}

/// Authenticated checkpoint; its signer is the operator already signed into the inventory.
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub(super) struct SignedHostPhaseV1 {
    pub(super) claims: HostPhaseClaimsV1,
    pub(super) signature_hex: String,
}

impl SignedHostPhaseV1 {
    pub(super) fn sign(claims: HostPhaseClaimsV1, operator: &KeyPair) -> Result<Self> {
        if operator.public_key().try_algorithm()? != Algorithm::Ed25519 {
            return Err(eyre!(
                "cross-host checkpoint signer must be the admitted Ed25519 operator"
            ));
        }
        let signature = Signature::try_new(operator.private_key(), &checkpoint_message(&claims)?)
            .map_err(|_| eyre!("host phase checkpoint signing failed"))?;
        Ok(Self {
            claims,
            signature_hex: hex::encode(signature.payload()),
        })
    }

    pub(super) fn digest(&self) -> Result<String> {
        let bytes = json::to_json(self)?;
        if bytes.len() > MAX_CHECKPOINT_BYTES {
            return Err(eyre!(
                "signed host checkpoint exceeds its finite wire bound"
            ));
        }
        Ok(sha256_hex(bytes.as_bytes()))
    }

    pub(super) fn verify(
        &self,
        inventory: &InventoryV1,
        inventory_sha256: &str,
        authorization_sha256: &str,
        execution_expires_at_unix_ms: u64,
        predecessor: Option<&SignedHostPhaseV1>,
    ) -> Result<()> {
        let claims = &self.claims;
        let hosts = &inventory.hosts;
        if claims.schema != CHECKPOINT_SCHEMA
            || claims.deployment_id != inventory.deployment_id
            || claims.inventory_sha256 != inventory_sha256
            || claims.authorization_sha256 != authorization_sha256
            || claims.authorization_nonce != inventory.authorization_nonce
            || claims.host_pair_sha256 != hosts.digest()?
            || claims.guest_host_identity_sha256
                != hosts.validator_guest.endpoint.host_identity_sha256
            || claims.native_edge_host_identity_sha256
                != hosts.native_edge.endpoint.host_identity_sha256
            || claims.guest_custody_root != hosts.validator_guest.custody_root
            || claims.native_edge_custody_root != hosts.native_edge.custody_root
            || claims.source_commit != inventory.revision.commit
            || claims.next_genesis_hash != inventory.next_genesis_hash
            || claims.execution_expires_at_unix_ms != execution_expires_at_unix_ms
        {
            return Err(eyre!(
                "signed checkpoint differs from the admitted host pair, inventory or authorization"
            ));
        }
        validate_lower_hex("checkpoint evidence", &claims.evidence_sha256, 64)?;
        match (claims.phase, predecessor, &claims.predecessor_sha256) {
            (HostPhaseV1::CandidateFrontier, None, None) => {}
            (phase, Some(previous), Some(digest))
                if phase.sequence() == previous.claims.phase.sequence() + 1
                    && digest == &previous.digest()? => {}
            _ => {
                return Err(eyre!(
                    "host phase checkpoint does not extend the exact ordered predecessor"
                ));
            }
        }
        validate_lower_hex("checkpoint signature", &self.signature_hex, 128)?;
        let signature = ed25519_parse_signature(&hex::decode(&self.signature_hex)?)?;
        let public_key = PublicKey::from_str(&inventory.operator_public_key)?;
        if public_key.try_algorithm()? != Algorithm::Ed25519 {
            return Err(eyre!(
                "cross-host checkpoint verifier requires the admitted Ed25519 operator"
            ));
        }
        verify_signature_for_admission(&signature, &public_key, &checkpoint_message(claims)?)
            .wrap_err("host phase checkpoint signature verification failed")?;
        self.digest()?;
        Ok(())
    }
}

fn checkpoint_message(claims: &HostPhaseClaimsV1) -> Result<Vec<u8>> {
    let body = json::to_json(claims)?;
    if body.len() > MAX_CHECKPOINT_BYTES - CHECKPOINT_DOMAIN.len() {
        return Err(eyre!(
            "host phase claims exceed their finite signature bound"
        ));
    }
    let mut message = Vec::with_capacity(CHECKPOINT_DOMAIN.len() + body.len());
    message.extend_from_slice(CHECKPOINT_DOMAIN);
    message.extend_from_slice(body.as_bytes());
    Ok(message)
}

/// Exact native file metadata. Only independently owned public content receives a digest.
#[derive(Clone, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub(super) struct NativeFileIdentityV1 {
    pub(super) device: u64,
    pub(super) inode: u64,
    pub(super) uid: u32,
    pub(super) gid: u32,
    pub(super) mode: u16,
    pub(super) links: u64,
    pub(super) size: u64,
    pub(super) mtime_ns: u64,
    pub(super) ctime_ns: u64,
}

/// Metadata-only native input: this type cannot represent a hash of private configuration.
#[derive(Clone, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub(super) struct NativeObservedFileV1 {
    pub(super) path: String,
    pub(super) identity: NativeFileIdentityV1,
}

/// Independently owned public metadata or rendered include, bounded and retained natively.
#[derive(Clone, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub(super) struct NativePublicFileV1 {
    pub(super) file: NativeObservedFileV1,
    pub(super) sha256: String,
}

/// Exact native nginx process identity; a PID alone cannot authorize a reload.
#[derive(Clone, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub(super) struct NativeNginxMasterV1 {
    pub(super) pid: u32,
    pub(super) uid: u32,
    pub(super) started: String,
    pub(super) executable: String,
}

/// Prior public include and journal belonging to one admitted native publisher.
#[derive(Clone, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub(super) struct NativeOwnedPublicationV1 {
    pub(super) operation_id: String,
    pub(super) journal: NativePublicFileV1,
    pub(super) publication: NativePublicFileV1,
}

/// A publication-only incumbent cannot stand in for a completed distributed reset.
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(tag = "kind", content = "value", deny_unknown_fields)]
pub(super) enum NativeEdgeCompletionProvenanceV1 {
    /// Genuine first publication has neither a prior CLI nor a publication receipt.
    #[norito(rename = "vacant")]
    Vacant,
    #[norito(rename = "publication_only")]
    PublicationOnly { publication_operation_id: String },
    #[norito(rename = "reset_terminal")]
    ResetTerminal {
        status: String,
        progress: NativePublicFileV1,
        completion_receipt: NativePublicFileV1,
        checkpoints: Vec<NativePublicFileV1>,
        #[norito(required)]
        global_proof: Option<NativePublicFileV1>,
        #[norito(required)]
        global_proof_predecessor: Option<NativePublicFileV1>,
    },
}

/// Actual retained native authority; a virgin edge has no predecessor pair.
#[derive(Clone, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub(super) struct NativeEdgeRetainedAuthorityV1 {
    pub(super) retained_inventory_sha256: String,
    pub(super) authorization_sha256: String,
}

impl NativeEdgeRetainedAuthorityV1 {
    pub(super) fn validate(&self) -> Result<()> {
        for digest in [&self.retained_inventory_sha256, &self.authorization_sha256] {
            validate_lower_hex("native predecessor authority", digest, 64)?;
            if digest.bytes().all(|byte| byte == b'0') {
                return Err(eyre!("native predecessor authority cannot use an invented zero digest"));
            }
        }
        Ok(())
    }
}

/// Independently captured native edge; Linux runtime capture never supplies these fields.
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub(super) struct NativeEdgeCaptureClaimsV1 {
    pub(super) schema: String,
    pub(super) host_pair_sha256: String,
    pub(super) host_identity_sha256: String,
    pub(super) owner_uid: u32,
    pub(super) owner_gid: u32,
    pub(super) custody_root: String,
    #[norito(required)]
    pub(super) predecessor_authority: Option<NativeEdgeRetainedAuthorityV1>,
    pub(super) authorization_nonce: String,
    pub(super) next_genesis_hash: String,
    pub(super) initial_state: EdgeInitialStateV1,
    pub(super) nginx_config: String,
    pub(super) dispatcher: NativePublicFileV1,
    pub(super) native_guard: NativePublicFileV1,
    pub(super) nginx: NativeObservedFileV1,
    pub(super) main_configuration: NativeObservedFileV1,
    pub(super) master: NativeNginxMasterV1,
    #[norito(required)]
    pub(super) owned_publication: Option<NativeOwnedPublicationV1>,
    pub(super) completion: NativeEdgeCompletionProvenanceV1,
    pub(super) forwarding_plan: NativePublicFileV1,
    pub(super) forwarding_identity_receipt: NativePublicFileV1,
    pub(super) forwarding_journal: NativePublicFileV1,
    pub(super) helper_source_closure_sha256: String,
    pub(super) captured_at_unix_ms: u64,
}

impl NativeEdgeCaptureClaimsV1 {
    pub(super) fn retained_authority(&self) -> Result<&NativeEdgeRetainedAuthorityV1> {
        self.predecessor_authority.as_ref()
            .ok_or_else(|| eyre!("vacant first publication has no retained native authority"))
    }

    pub(super) fn validate_predecessor_authority(&self) -> Result<()> {
        match (&self.completion, &self.predecessor_authority) {
            (NativeEdgeCompletionProvenanceV1::Vacant, None)
                if matches!(self.initial_state, EdgeInitialStateV1::Vacant)
                    && self.owned_publication.is_none() => Ok(()),
            (NativeEdgeCompletionProvenanceV1::PublicationOnly { .. }, Some(authority))
            | (NativeEdgeCompletionProvenanceV1::ResetTerminal { .. }, Some(authority)) => {
                authority.validate()
            }
            _ => Err(eyre!("native predecessor authority must match its actual completion provenance")),
        }
    }

    pub(super) fn admitted_release(&self) -> Result<&EdgeAdmittedReleaseV1> {
        match &self.initial_state {
            EdgeInitialStateV1::AdmittedRelease(release) => Ok(release),
            EdgeInitialStateV1::Vacant => Err(eyre!("vacant native edge has no prior release")),
        }
    }
    pub(super) fn owned_publication(&self) -> Result<&NativeOwnedPublicationV1> {
        self.owned_publication.as_ref()
            .ok_or_else(|| eyre!("vacant native edge has no prior publication"))
    }
}

/// Native software custody signature over the complete edge predecessor capture.
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub(super) struct SignedNativeEdgeCaptureV1 {
    pub(super) claims: NativeEdgeCaptureClaimsV1,
    pub(super) signature_hex: String,
}

impl SignedNativeEdgeCaptureV1 {
    pub(super) fn sign(claims: NativeEdgeCaptureClaimsV1, native_key: &KeyPair) -> Result<Self> {
        if native_key.public_key().try_algorithm()? != Algorithm::Ed25519 {
            return Err(eyre!(
                "native capture signer must be the admitted Ed25519 software custodian"
            ));
        }
        let body = json::to_json(&claims)?;
        if body.len() > 64 * 1024 {
            return Err(eyre!("native capture exceeds its finite signature bound"));
        }
        let mut message = b"iroha:taira:public-reset:native-edge-capture:v1\0".to_vec();
        message.extend_from_slice(body.as_bytes());
        let signature = Signature::try_new(native_key.private_key(), &message)
            .map_err(|_| eyre!("native software custody signing failed"))?;
        Ok(Self {
            claims,
            signature_hex: hex::encode(signature.payload()),
        })
    }

    /// Keep independently authenticated guest/lease authority required for runtime joins.
    pub(super) fn verify_retained_join(
        &self,
        hosts: &ResetHostPairV1,
        retained_inventory_sha256: &str,
        authorization_sha256: &str,
        authorization_nonce: &str,
        next_genesis_hash: &str,
    ) -> Result<()> {
        let actual = NativeEdgeRetainedAuthorityV1 {
            retained_inventory_sha256: retained_inventory_sha256.into(),
            authorization_sha256: authorization_sha256.into(),
        };
        actual.validate()?;
        self.verify(
            hosts,
            self.claims.predecessor_authority.as_ref().map(|_| &actual),
            authorization_nonce,
            next_genesis_hash,
        )
    }

    pub(super) fn verify(
        &self,
        hosts: &ResetHostPairV1,
        predecessor_authority: Option<&NativeEdgeRetainedAuthorityV1>,
        authorization_nonce: &str,
        next_genesis_hash: &str,
    ) -> Result<()> {
        hosts.validate()?;
        self.claims.validate_predecessor_authority()?;
        let host = &hosts.native_edge;
        let claims = &self.claims;
        if claims.schema != "iroha.taira.public-reset.native-edge-capture.v1"
            || claims.host_pair_sha256 != hosts.digest()?
            || claims.host_identity_sha256 != host.endpoint.host_identity_sha256
            || claims.owner_uid != host.owner_uid
            || claims.owner_gid != host.owner_gid
            || claims.custody_root != host.custody_root
            || claims.predecessor_authority.as_ref() != predecessor_authority
            || claims.authorization_nonce != authorization_nonce
            || claims.next_genesis_hash != next_genesis_hash
            || claims.dispatcher.file.path != host.dispatcher_path
            || claims.dispatcher.sha256 != host.dispatcher_sha256
            || claims.native_guard.file.path
                != format!("{}/taira-edge/guard.json", host.custody_root)
            || claims.native_guard.sha256 != host.guard_sha256
            || claims.native_guard.file.identity.uid != host.owner_uid
            || claims.native_guard.file.identity.mode != 0o600
            || claims.master.uid != host.owner_uid
            || claims.master.executable != claims.nginx.path
            || claims.master.pid < 2
            || claims.master.started.is_empty()
            || claims.captured_at_unix_ms == 0
        {
            return Err(eyre!(
                "native edge capture differs from the admitted host, predecessor or custodian"
            ));
        }
        if claims.helper_source_closure_sha256 != helper_source_closure_sha256() {
            return Err(eyre!(
                "native edge capture names another maintained helper source closure"
            ));
        }
        validate_absolute_normal_path(Path::new(&claims.nginx_config), "native publication destination")?;
        match (&claims.initial_state, &claims.owned_publication, &claims.completion) {
            (EdgeInitialStateV1::Vacant, None, NativeEdgeCompletionProvenanceV1::Vacant) => {},
            (EdgeInitialStateV1::Vacant, None, NativeEdgeCompletionProvenanceV1::ResetTerminal { status, .. })
                if status == "rolled_back" => {},
            (EdgeInitialStateV1::AdmittedRelease(release), Some(publication), completion)
                if !matches!(completion, NativeEdgeCompletionProvenanceV1::Vacant) => {
                validate_lower_hex("native edge predecessor commit", &release.commit, 40)?;
                validate_lower_hex("native edge predecessor CLI", &release.cli_sha256, 64)?;
                validate_lower_hex("native edge predecessor public config", &release.config_sha256, 64)?;
                if release.release_root != format!(
                    "{}/.local/share/iroha/taira/edge/releases/{}", host.owner_home, release.commit
                ) || release.config_sha256 != publication.publication.sha256
                    || claims.nginx_config != publication.publication.file.path
                {
                    return Err(eyre!("native edge predecessor release or public publication binding differs"));
                }
                validate_lower_hex("native publisher operation", &publication.operation_id, 32)?;
            }
            _ => return Err(eyre!("native initial occupancy and publication provenance disagree")),
        }
        match &claims.completion {
            NativeEdgeCompletionProvenanceV1::Vacant => {},
            NativeEdgeCompletionProvenanceV1::PublicationOnly {
                publication_operation_id,
            } if publication_operation_id == &claims.owned_publication()?.operation_id => {}
            NativeEdgeCompletionProvenanceV1::ResetTerminal {
                status,
                progress,
                completion_receipt,
                checkpoints,
                global_proof,
                global_proof_predecessor,
            } => {
                let terminal_root = format!(
                    "{}/taira-edge/operations/{}",
                    host.custody_root, claims.retained_authority()?.authorization_sha256
                );
                let proven = matches!(status.as_str(), "sealed" | "cleaned");
                if !(proven || status == "rolled_back")
                    || proven != global_proof.is_some()
                    || proven != global_proof_predecessor.is_some()
                    || (proven && checkpoints.len() != 3)
                    || (!proven && checkpoints.len() > 2)
                {
                    return Err(eyre!(
                        "native terminal provenance lacks the exact distributed proof boundary"
                    ));
                }
                for reference in std::iter::once(progress)
                    .chain(std::iter::once(completion_receipt))
                    .chain(checkpoints.iter())
                    .chain(global_proof.iter())
                    .chain(global_proof_predecessor.iter())
                {
                    reference.validate_public(host.owner_uid)?;
                    if reference.file.identity.uid != host.owner_uid
                        || reference.file.identity.mode != 0o600
                        || reference.file.identity.size > 64 * 1024
                        || !Path::new(&reference.file.path).starts_with(&terminal_root)
                    {
                        return Err(eyre!(
                            "native terminal proof escaped its bounded per-authorization custody"
                        ));
                    }
                }
                if global_proof.as_ref().is_some_and(|proof| {
                    proof.file.path != format!("{terminal_root}/global-proof.json")
                }) {
                    return Err(eyre!("native terminal fence has another anchored pathname"));
                }
                if global_proof_predecessor.as_ref().is_some_and(|proof| {
                    proof.file.path != format!("{terminal_root}/global-proof-predecessor.json")
                }) {
                    return Err(eyre!(
                        "native terminal pre-fence snapshot has another anchored pathname"
                    ));
                }
            }
            _ => return Err(eyre!("native publication provenance names another owner")),
        }
        for reference in [
            &claims.dispatcher,
            &claims.native_guard,
            &claims.forwarding_plan,
            &claims.forwarding_identity_receipt,
            &claims.forwarding_journal,
        ] {
            reference.validate_public(host.owner_uid)?;
        }
        for publication in &claims.owned_publication {
            publication.journal.validate_public(host.owner_uid)?;
            publication.publication.validate_public(host.owner_uid)?;
        }
        for reference in [&claims.nginx, &claims.main_configuration] {
            reference.validate_metadata(host.owner_uid)?;
        }
        validate_lower_hex("native capture signature", &self.signature_hex, 128)?;
        let signature = ed25519_parse_signature(&hex::decode(&self.signature_hex)?)?;
        let public_key = PublicKey::from_str(&host.capture_public_key)?;
        let body = json::to_json(claims)?;
        if body.len() > 64 * 1024 {
            return Err(eyre!("native edge capture exceeded its finite wire bound"));
        }
        let mut message = b"iroha:taira:public-reset:native-edge-capture:v1\0".to_vec();
        message.extend_from_slice(body.as_bytes());
        verify_signature_for_admission(&signature, &public_key, &message)
            .wrap_err("native edge capture software custody signature verification failed")
    }
}

impl NativeObservedFileV1 {
    pub(super) fn validate_metadata(&self, owner_uid: u32) -> Result<()> {
        validate_absolute_normal_path(Path::new(&self.path), "native captured file")?;
        let identity = &self.identity;
        if !matches!(identity.uid, 0) && identity.uid != owner_uid
            || identity.mode & 0o022 != 0
            || identity.mode & !0o777 != 0
            || identity.links != 1
            || identity.inode == 0
        {
            return Err(eyre!(
                "native captured file has unsafe owner, mode or link identity"
            ));
        }
        Ok(())
    }
}

impl NativePublicFileV1 {
    pub(super) fn validate_public(&self, owner_uid: u32) -> Result<()> {
        self.file.validate_metadata(owner_uid)?;
        validate_lower_hex("native public file SHA-256", &self.sha256, 64)?;
        if self.file.identity.size == 0 || self.file.identity.size > 512 * 1024 * 1024 {
            return Err(eyre!(
                "native public file exceeds the admitted finite artifact bound"
            ));
        }
        Ok(())
    }
}

/// Source- and host-bound Darwin release authorization from the independently trusted owner.
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub(super) struct NativeEdgeCandidateClaimsV1 {
    pub(super) schema: String,
    pub(super) commit: String,
    pub(super) tree: String,
    pub(super) cargo_lock_sha256: String,
    pub(super) source_closure_sha256: String,
    pub(super) target: String,
    pub(super) host_pair_sha256: String,
    pub(super) iroha_cli: ArtifactV1,
    /// Already admitted native guard, captured on the Mac after its native dispatcher preparation.
    pub(super) native_guard: NativePublicFileV1,
    pub(super) helper_source_closure_sha256: String,
}

/// Native candidate authorization cannot be replaced by an unsigned path or Linux import.
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub(super) struct SignedNativeEdgeCandidateV1 {
    pub(super) claims: NativeEdgeCandidateClaimsV1,
    pub(super) signature_hex: String,
}

impl SignedNativeEdgeCandidateV1 {
    pub(super) fn sign(claims: NativeEdgeCandidateClaimsV1, owner: &KeyPair) -> Result<Self> {
        if owner.public_key().try_algorithm()? != Algorithm::Ed25519 {
            return Err(eyre!(
                "native candidate signer must be the independent Ed25519 release owner"
            ));
        }
        let body = json::to_json(&claims)?;
        if body.len() > 16 * 1024 {
            return Err(eyre!("native candidate exceeds its finite signature bound"));
        }
        let mut message = b"iroha:taira:public-reset:native-edge-candidate:v1\0".to_vec();
        message.extend_from_slice(body.as_bytes());
        let signature = Signature::try_new(owner.private_key(), &message)
            .map_err(|_| eyre!("native Darwin release owner signing failed"))?;
        Ok(Self {
            claims,
            signature_hex: hex::encode(signature.payload()),
        })
    }

    pub(super) fn verify(
        &self,
        hosts: &ResetHostPairV1,
        commit: &str,
        tree: &str,
        cargo_lock_sha256: &str,
        source_closure_sha256: &str,
        trusted: &TrustedKeyV1,
    ) -> Result<()> {
        hosts.validate()?;
        let claims = &self.claims;
        let native = &hosts.native_edge;
        let cli = &claims.iroha_cli;
        if claims.schema != "iroha.taira.public-reset.native-edge-candidate.v1"
            || claims.commit != commit
            || claims.tree != tree
            || claims.cargo_lock_sha256 != cargo_lock_sha256
            || claims.source_closure_sha256 != source_closure_sha256
            || claims.target != NATIVE_EDGE_TARGET
            || cli.target != NATIVE_EDGE_TARGET
            || claims.host_pair_sha256 != hosts.digest()?
            || claims.helper_source_closure_sha256 != helper_source_closure_sha256()
            || cli.role != "iroha_cli"
            || cli.source_commit != commit
            || cli.sha256 != native.dispatcher_sha256
            || cli.mode != 0o755
            || cli.size < 64
            || cli.size > 512 * 1024 * 1024
            || cli.remote_path
                != format!(
                    "{}/.local/share/iroha/taira/edge/releases/{commit}/bin/iroha",
                    native.owner_home
                )
            || claims.native_guard.file.path
                != format!("{}/taira-edge/guard.json", native.custody_root)
            || claims.native_guard.sha256 != native.guard_sha256
            || claims.native_guard.file.identity.uid != native.owner_uid
            || claims.native_guard.file.identity.mode != 0o600
        {
            return Err(eyre!(
                "native Darwin candidate differs from the signed source, target or prepared Mac custodian"
            ));
        }
        validate_absolute_normal_path(
            Path::new(&cli.local_path),
            "native candidate public projection",
        )?;
        validate_lower_hex("native candidate artifact SHA-256", &cli.sha256, 64)?;
        claims.native_guard.validate_public(native.owner_uid)?;
        validate_lower_hex("native candidate signature", &self.signature_hex, 128)?;
        let key = PublicKey::from_str(&trusted.public_key)?;
        if trusted.schema != TRUSTED_KEY_SCHEMA_V1 || key.try_algorithm()? != Algorithm::Ed25519 {
            return Err(eyre!(
                "native candidate requires the independent Ed25519 reset authorization owner"
            ));
        }
        let signature = ed25519_parse_signature(&hex::decode(&self.signature_hex)?)?;
        let body = json::to_json(claims)?;
        if body.len() > 16 * 1024 {
            return Err(eyre!(
                "native candidate exceeds its finite owner signature bound"
            ));
        }
        let mut message = b"iroha:taira:public-reset:native-edge-candidate:v1\0".to_vec();
        message.extend_from_slice(body.as_bytes());
        verify_signature_for_admission(&signature, &key, &message)
            .wrap_err("native Darwin candidate owner authorization verification failed")
    }
}

#[cfg(test)]
pub(super) fn fixture_pair() -> ResetHostPairV1 {
    let endpoint = |native: bool| HostSshRouteV1 {
        hostname: if native {
            "taira-mac.example.org"
        } else {
            "taira-guest.example.org"
        }
        .into(),
        port: 22,
        user: if native { "taira" } else { "root" }.into(),
        known_host_line_sha256: if native { "a" } else { "b" }.repeat(64),
        host_identity_sha256: if native { "c" } else { "d" }.repeat(64),
    };
    ResetHostPairV1 {
        schema: HOST_PAIR_SCHEMA.into(),
        provider: "macstadium-dublin".into(),
        validator_guest: ResetHostV1 {
            endpoint: endpoint(false),
            platform: PlatformV1 {
                os: "linux".into(),
                arch: "aarch64".into(),
                kvm_api_version: 12,
            },
            owner_uid: 0,
            owner_gid: 0,
            owner_home: "/root".into(),
            custody_root: "/var/lib/taira/.public-reset-control-v1".into(),
            dispatcher_path: "/usr/local/libexec/iroha-taira-public-reset-v1".into(),
            dispatcher_sha256: "f".repeat(64),
            guard_sha256: "e".repeat(64),
            capture_public_key: KeyPair::from_seed(
                b"guest-native-capture".to_vec(),
                Algorithm::Ed25519,
            )
            .public_key()
            .to_string(),
        },
        native_edge: ResetHostV1 {
            endpoint: endpoint(true),
            platform: PlatformV1 {
                os: "macos".into(),
                arch: "aarch64".into(),
                kvm_api_version: 0,
            },
            owner_uid: 501,
            owner_gid: 20,
            owner_home: "/Users/taira".into(),
            custody_root: "/Users/taira/.local/share/iroha/taira/public-reset-v1".into(),
            dispatcher_path:
                "/Users/taira/.local/share/iroha/taira/public-reset-v1/dispatcher/iroha".into(),
            dispatcher_sha256: "1".repeat(64),
            guard_sha256: "2".repeat(64),
            capture_public_key: KeyPair::from_seed(
                b"mac-native-capture".to_vec(),
                Algorithm::Ed25519,
            )
            .public_key()
            .to_string(),
        },
    }
}

#[cfg(test)]
pub(super) fn fixture_native_edge_capture(
    hosts: &ResetHostPairV1,
    release: EdgeAdmittedReleaseV1,
    inventory_sha256: &str,
    authorization_sha256: &str,
    nonce: &str,
    genesis_hash: &str,
) -> SignedNativeEdgeCaptureV1 {
    let host = &hosts.native_edge;
    let public = |path: String, sha256: String, mode: u16, inode: u64| NativePublicFileV1 {
        file: NativeObservedFileV1 {
            path,
            identity: NativeFileIdentityV1 {
                device: 1,
                inode,
                uid: host.owner_uid,
                gid: host.owner_gid,
                mode,
                links: 1,
                size: 64,
                mtime_ns: 1,
                ctime_ns: 1,
            },
        },
        sha256,
    };
    let operation_id = "a".repeat(32);
    let publication = public(
        "/opt/homebrew/etc/nginx/servers/taira-public-validator-listeners.conf".into(),
        release.config_sha256.clone(),
        0o640,
        4,
    );
    let claims = NativeEdgeCaptureClaimsV1 {
        schema: "iroha.taira.public-reset.native-edge-capture.v1".into(),
        host_pair_sha256: hosts.digest().unwrap(),
        host_identity_sha256: host.endpoint.host_identity_sha256.clone(),
        owner_uid: host.owner_uid,
        owner_gid: host.owner_gid,
        custody_root: host.custody_root.clone(),
        predecessor_authority: Some(NativeEdgeRetainedAuthorityV1 {
            retained_inventory_sha256: inventory_sha256.into(),
            authorization_sha256: authorization_sha256.into(),
        }),
        authorization_nonce: nonce.into(),
        next_genesis_hash: genesis_hash.into(),
        initial_state: EdgeInitialStateV1::AdmittedRelease(release),
        nginx_config: publication.file.path.clone(),
        dispatcher: public(
            host.dispatcher_path.clone(),
            host.dispatcher_sha256.clone(),
            0o755,
            1,
        ),
        native_guard: public(
            format!("{}/taira-edge/guard.json", host.custody_root),
            host.guard_sha256.clone(),
            0o600,
            9,
        ),
        nginx: public(
            "/opt/homebrew/opt/nginx/bin/nginx".into(),
            "b".repeat(64),
            0o755,
            2,
        )
        .file,
        main_configuration: public(
            "/opt/homebrew/etc/nginx/nginx.conf".into(),
            "c".repeat(64),
            0o644,
            3,
        )
        .file,
        master: NativeNginxMasterV1 {
            pid: 1024,
            uid: host.owner_uid,
            started: "Mon Oct  5 12:00:00 2026".into(),
            executable: "/opt/homebrew/opt/nginx/bin/nginx".into(),
        },
        owned_publication: Some(NativeOwnedPublicationV1 {
            journal: public(
                format!(
                    "/opt/homebrew/etc/nginx/.taira-native-nginx-apply-{operation_id}.receipt.ndjson"
                ),
                "d".repeat(64),
                0o600,
                5,
            ),
            operation_id: operation_id.clone(),
            publication,
        }),
        completion: NativeEdgeCompletionProvenanceV1::PublicationOnly {
            publication_operation_id: operation_id,
        },
        forwarding_plan: public(
            format!("{}/native-forwarding-plan.json", host.custody_root),
            "e".repeat(64),
            0o600,
            6,
        ),
        forwarding_identity_receipt: public(
            format!("{}/native-forwarding-identity.json", host.custody_root),
            "f".repeat(64),
            0o600,
            7,
        ),
        forwarding_journal: public(
            format!("{}/native-forwarding.receipt.ndjson", host.custody_root),
            "1".repeat(64),
            0o600,
            8,
        ),
        helper_source_closure_sha256: helper_source_closure_sha256(),
        captured_at_unix_ms: 123_000,
    };
    SignedNativeEdgeCaptureV1::sign(
        claims,
        &KeyPair::from_seed(b"mac-native-capture".to_vec(), Algorithm::Ed25519),
    )
    .unwrap()
}

#[cfg(test)]
pub(super) fn fixture_native_edge_candidate(
    hosts: &ResetHostPairV1,
    revision: &RevisionV1,
    cli_local_path: String,
) -> SignedNativeEdgeCandidateV1 {
    let host = &hosts.native_edge;
    let claims = NativeEdgeCandidateClaimsV1 {
        schema: "iroha.taira.public-reset.native-edge-candidate.v1".into(),
        commit: revision.commit.clone(),
        tree: revision.tree.clone(),
        cargo_lock_sha256: revision.cargo_lock_sha256.clone(),
        source_closure_sha256: revision.source_closure_sha256.clone(),
        target: NATIVE_EDGE_TARGET.into(),
        host_pair_sha256: hosts.digest().unwrap(),
        iroha_cli: ArtifactV1 {
            role: "iroha_cli".into(),
            local_path: cli_local_path,
            remote_path: format!(
                "{}/.local/share/iroha/taira/edge/releases/{}/bin/iroha",
                host.owner_home, revision.commit
            ),
            sha256: host.dispatcher_sha256.clone(),
            size: 64,
            mode: 0o755,
            source_commit: revision.commit.clone(),
            target: NATIVE_EDGE_TARGET.into(),
        },
        native_guard: NativePublicFileV1 {
            file: NativeObservedFileV1 {
                path: format!("{}/taira-edge/guard.json", host.custody_root),
                identity: NativeFileIdentityV1 {
                    device: 1,
                    inode: 9,
                    uid: host.owner_uid,
                    gid: host.owner_gid,
                    mode: 0o600,
                    links: 1,
                    size: 64,
                    mtime_ns: 1,
                    ctime_ns: 1,
                },
            },
            sha256: host.guard_sha256.clone(),
        },
        helper_source_closure_sha256: helper_source_closure_sha256(),
    };
    SignedNativeEdgeCandidateV1::sign(
        claims,
        &KeyPair::from_seed(b"native-candidate-owner".to_vec(), Algorithm::Ed25519),
    )
    .unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn checkpoint(
        phase: HostPhaseV1,
        previous: Option<&SignedHostPhaseV1>,
    ) -> (InventoryV1, KeyPair, SignedHostPhaseV1) {
        let mut inventory = sample_inventory_fixture();
        let key = KeyPair::from_seed(b"host-pair-phase-fixture".to_vec(), Algorithm::Ed25519);
        inventory.operator_public_key = key.public_key().to_string();
        let hosts = &inventory.hosts;
        let claims = HostPhaseClaimsV1 {
            schema: CHECKPOINT_SCHEMA.into(),
            phase,
            deployment_id: inventory.deployment_id.clone(),
            inventory_sha256: "2".repeat(64),
            authorization_sha256: "3".repeat(64),
            authorization_nonce: inventory.authorization_nonce.clone(),
            host_pair_sha256: hosts.digest().unwrap(),
            guest_host_identity_sha256: hosts.validator_guest.endpoint.host_identity_sha256.clone(),
            native_edge_host_identity_sha256: hosts
                .native_edge
                .endpoint
                .host_identity_sha256
                .clone(),
            guest_custody_root: hosts.validator_guest.custody_root.clone(),
            native_edge_custody_root: hosts.native_edge.custody_root.clone(),
            source_commit: inventory.revision.commit.clone(),
            next_genesis_hash: inventory.next_genesis_hash.clone(),
            evidence_sha256: "4".repeat(64),
            predecessor_sha256: previous.map(|p| p.digest().unwrap()),
            execution_expires_at_unix_ms: 123_000,
        };
        let signed = SignedHostPhaseV1::sign(claims, &key).unwrap();
        (inventory, key, signed)
    }

    #[test]
    fn canonical_pair_rejects_provider_platform_owner_and_same_host_substitution() {
        let pair = sample_inventory_fixture().hosts;
        pair.validate().unwrap();
        for mutation in 0..5 {
            let mut changed = pair.clone();
            match mutation {
                0 => changed.provider = "aws".into(),
                1 => changed.native_edge.platform.os = "linux".into(),
                2 => changed.native_edge.owner_uid = 0,
                3 => {
                    changed.native_edge.custody_root = changed.validator_guest.custody_root.clone()
                }
                _ => {
                    changed.native_edge.endpoint.host_identity_sha256 = changed
                        .validator_guest
                        .endpoint
                        .host_identity_sha256
                        .clone()
                }
            }
            assert!(changed.validate().is_err());
        }
    }

    #[test]
    fn signed_frontier_is_bound_to_both_hosts_custody_and_reset_identity() {
        let (inventory, _, signed) = checkpoint(HostPhaseV1::CandidateFrontier, None);
        signed
            .verify(&inventory, &"2".repeat(64), &"3".repeat(64), 123_000, None)
            .unwrap();
        for mutation in 0..5 {
            let mut changed = signed.clone();
            match mutation {
                0 => changed.claims.native_edge_host_identity_sha256 = "5".repeat(64),
                1 => changed.claims.guest_custody_root.push_str("/foreign"),
                2 => changed.claims.authorization_nonce.push('x'),
                3 => changed.claims.evidence_sha256 = "6".repeat(64),
                _ => changed.signature_hex = "0".repeat(128),
            }
            assert!(
                changed
                    .verify(&inventory, &"2".repeat(64), &"3".repeat(64), 123_000, None)
                    .is_err()
            );
        }
    }

    #[test]
    fn native_cutover_and_global_proof_require_the_exact_ordered_checkpoint_chain() {
        let (inventory, _, first) = checkpoint(HostPhaseV1::CandidateFrontier, None);
        let (_, _, second) = checkpoint(HostPhaseV1::NativeEdgeReady, Some(&first));
        let (_, _, third) = checkpoint(HostPhaseV1::DeploymentProven, Some(&second));
        let complete = vec![first.clone(), second.clone(), third.clone()];
        verify_checkpoint_chain(
            &complete,
            &inventory,
            &"2".repeat(64),
            &"3".repeat(64),
            123_000,
            HostPhaseV1::DeploymentProven,
        )
        .unwrap();
        assert!(
            verify_checkpoint_chain(
                &complete[1..],
                &inventory,
                &"2".repeat(64),
                &"3".repeat(64),
                123_000,
                HostPhaseV1::DeploymentProven
            )
            .is_err()
        );
        second
            .verify(
                &inventory,
                &"2".repeat(64),
                &"3".repeat(64),
                123_000,
                Some(&first),
            )
            .unwrap();
        third
            .verify(
                &inventory,
                &"2".repeat(64),
                &"3".repeat(64),
                123_000,
                Some(&second),
            )
            .unwrap();
        assert!(
            second
                .verify(&inventory, &"2".repeat(64), &"3".repeat(64), 123_000, None)
                .is_err()
        );
        assert!(
            third
                .verify(
                    &inventory,
                    &"2".repeat(64),
                    &"3".repeat(64),
                    123_000,
                    Some(&first)
                )
                .is_err()
        );
        let mut foreign = first.clone();
        foreign.claims.evidence_sha256 = "7".repeat(64);
        assert!(
            second
                .verify(
                    &inventory,
                    &"2".repeat(64),
                    &"3".repeat(64),
                    123_000,
                    Some(&foreign)
                )
                .is_err()
        );
        let (_, key, mut corrupted) = checkpoint(HostPhaseV1::CandidateFrontier, None);
        corrupted.signature_hex = "0".repeat(128);
        // Re-sign valid descendants over the invalid predecessor's bytes: every
        // member still requires its own signature, not just a matching digest.
        let mut descendant_claims = second.claims.clone();
        descendant_claims.predecessor_sha256 = Some(corrupted.digest().unwrap());
        let descendant = SignedHostPhaseV1::sign(descendant_claims, &key).unwrap();
        assert!(
            verify_checkpoint_chain(
                &[corrupted, descendant],
                &inventory,
                &"2".repeat(64),
                &"3".repeat(64),
                123_000,
                HostPhaseV1::NativeEdgeReady
            )
            .is_err()
        );
    }
}
