//! Native edge custody passed only from the authenticated Darwin dispatcher.
//!
//! Rust verifies signatures and the complete cross-host checkpoint chain. The
//! maintained native helper receives inherited, retained descriptors and checks
//! their native parent, lock, pathname and byte identities before each effect.

use super::*;
use host_pair::{NativeObservedFileV1, NativePublicFileV1, SignedHostPhaseV1};
#[cfg(unix)]
use std::os::unix::fs::MetadataExt as _;

pub(super) const PROGRESS_SCHEMA: &str = "iroha.taira.public-reset.native-edge-progress.v1";
pub(super) const FENCE_SCHEMA: &str = "iroha.taira.public-reset.native-edge-global-proof.v1";
pub(super) const ADMISSION_SCHEMA: &str =
    "iroha.taira.public-reset.native-edge-completion-admission.v1";
pub(super) const MAX_PROGRESS_BYTES: usize = 16 * 1024;
pub(super) const MAX_COMPLETION_ADMISSION_BYTES: usize = 64 * 1024;

/// Required native inputs. The separately signed capture retains its own exact
/// custodian generation while guest successor pins may advance independently.
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub(super) struct NativeEdgeCapabilityV1 {
    pub(super) schema: String,
    pub(super) captured_hosts: host_pair::ResetHostPairV1,
    pub(super) host_pair_sha256: String,
    pub(super) helper_source_closure_sha256: String,
    pub(super) incumbent: host_pair::SignedNativeEdgeCaptureV1,
    #[norito(required)]
    pub(super) incumbent_nginx_request: Option<NativePublicFileV1>,
    pub(super) nginx_apply_plan: NativePublicFileV1,
    pub(super) forwarding_plan: NativePublicFileV1,
    pub(super) forwarding_identity_receipt: NativePublicFileV1,
}

impl NativeEdgeCapabilityV1 {
    pub(super) fn validate(&self, hosts: &host_pair::ResetHostPairV1) -> Result<()> {
        self.captured_hosts.validate_physical_binding(hosts)?;
        let captured = &self.captured_hosts.native_edge;
        let native = &hosts.native_edge;
        if self.schema != "iroha.taira.public-reset.native-edge-capability.v1"
            || self.host_pair_sha256 != self.captured_hosts.digest()?
            || self.helper_source_closure_sha256 != host_pair::helper_source_closure_sha256()
            || captured.dispatcher_sha256 != native.dispatcher_sha256
            || captured.guard_sha256 != native.guard_sha256
            || self.incumbent.claims.helper_source_closure_sha256
                != self.helper_source_closure_sha256
            || self.forwarding_plan != self.incumbent.claims.forwarding_plan
            || self.forwarding_identity_receipt != self.incumbent.claims.forwarding_identity_receipt
        {
            return Err(eyre!(
                "native edge capability changes its actual native custodian or captured helper closure"
            ));
        }
        let claims = &self.incumbent.claims;
        self.incumbent.verify(
            &self.captured_hosts,
            claims.predecessor_authority.as_ref(),
            &claims.authorization_nonce,
            &claims.next_genesis_hash,
        )?;
        if matches!(&claims.initial_state, EdgeInitialStateV1::Vacant)
            != self.incumbent_nginx_request.is_none()
        {
            return Err(eyre!(
                "native capability incumbent request must match its exact captured initial state"
            ));
        }
        for reference in self.incumbent_nginx_request.iter().chain([
            &self.nginx_apply_plan,
            &self.forwarding_plan,
            &self.forwarding_identity_receipt,
        ]) {
            reference.validate_public(native.owner_uid)?;
            if reference.file.identity.uid != native.owner_uid
                || reference.file.identity.mode != 0o600
                || reference.file.identity.size > 1024 * 1024
                || !Path::new(&reference.file.path).starts_with(&native.custody_root)
            {
                return Err(eyre!(
                    "native capability public input escaped its bounded native software custody"
                ));
            }
        }
        Ok(())
    }
}

/// A terminal helper reconciles the actual owned publication under its existing lock.
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub(super) struct NativeNginxCompletionPlanV1 {
    pub(super) schema: String,
    pub(super) nginx: Value,
    pub(super) publication_effect: NativePublicationEffectV1,
    pub(super) completion_journal_basename: String,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(tag = "kind", content = "value", deny_unknown_fields)]
pub(super) enum NativePublicationEffectV1 {
    #[norito(rename = "owned")]
    Owned,
    #[norito(rename = "not_requested")]
    NotRequested {
        intended_operation_id: String,
        incumbent: host_pair::NativeOwnedPublicationV1,
    },
    #[norito(rename = "vacant_not_requested")]
    VacantNotRequested {
        intended_operation_id: String,
    },
    #[norito(rename = "vacant_publisher_terminal")]
    VacantPublisherTerminal {
        intended_operation_id: String,
        publisher_journal: NativePublicFileV1,
        terminal_phase: String,
    },
}

impl NativeNginxCompletionPlanV1 {
    pub(super) fn validate(&self, publication_operation_id: &str) -> Result<()> {
        validate_lower_hex("completion publisher", publication_operation_id, 32)?;
        let plan = self
            .nginx
            .as_object()
            .ok_or_else(|| eyre!("native completion nginx plan is not an object"))?;
        let publication = plan
            .get("publication")
            .and_then(Value::as_object)
            .ok_or_else(|| eyre!("native completion lacks its exact owned publisher"))?;
        let publisher = match &self.publication_effect {
            NativePublicationEffectV1::Owned => publication_operation_id,
            NativePublicationEffectV1::NotRequested {
                intended_operation_id,
                incumbent,
            } => {
                if intended_operation_id != publication_operation_id
                    || incumbent.operation_id == *intended_operation_id
                {
                    return Err(eyre!(
                        "no-effect rollback changed its intended or incumbent owner"
                    ));
                }
                incumbent.operation_id.as_str()
            }
            NativePublicationEffectV1::VacantNotRequested {
                intended_operation_id,
            }
            | NativePublicationEffectV1::VacantPublisherTerminal {
                intended_operation_id, ..
            } => {
                if intended_operation_id != publication_operation_id
                    || publication.len() != 1
                    || publication.get("kind").and_then(Value::as_str) != Some("create")
                {
                    return Err(eyre!(
                        "vacant no-effect rollback changed its exact intended first publication"
                    ));
                }
                publication_operation_id
            }
        };
        let exact_publication = match &self.publication_effect {
            NativePublicationEffectV1::VacantNotRequested { .. }
            | NativePublicationEffectV1::VacantPublisherTerminal { .. } => true,
            NativePublicationEffectV1::Owned | NativePublicationEffectV1::NotRequested { .. } => {
                publication.get("kind").and_then(Value::as_str) == Some("reconcile")
                    && publication.get("prior").and_then(Value::as_object)
                        .and_then(|prior| prior.get("operation_id"))
                        .and_then(Value::as_str) == Some(publisher)
            }
        };
        if self.schema != "iroha.taira.public-reset.native-nginx-completion-plan.v1"
            || self.completion_journal_basename != "native-completion.ndjson"
            || plan.get("schema").and_then(Value::as_str)
                != Some("iroha.taira.native-nginx-apply.plan.v1")
            || plan.get("host_kind").and_then(Value::as_str) != Some("macos")
            || plan.get("provider").and_then(Value::as_str) != Some("macstadium-dublin")
            || plan.get("operation_id").and_then(Value::as_str) != Some(publisher)
            || !exact_publication
        {
            return Err(eyre!(
                "native completion plan changes its exact admitted publication effect"
            ));
        }
        if let NativePublicationEffectV1::VacantPublisherTerminal {
            intended_operation_id, publisher_journal, terminal_phase,
        } = &self.publication_effect {
            let native = plan.get("native").and_then(Value::as_object)
                .ok_or_else(|| eyre!("terminal vacancy lacks its native publisher custody"))?;
            let owner = native.get("owner_uid").and_then(Value::as_u64)
                .and_then(|value| u32::try_from(value).ok())
                .ok_or_else(|| eyre!("terminal vacancy lacks its native publisher owner"))?;
            let directory = native.get("directory").and_then(|value| value.get("path"))
                .and_then(Value::as_str)
                .ok_or_else(|| eyre!("terminal vacancy lacks its exact journal directory"))?;
            validate_absolute_normal_path(Path::new(directory), "terminal publisher journal directory")?;
            publisher_journal.validate_public(owner)?;
            if !matches!(terminal_phase.as_str(), "rolled_back_unqualified" | "refused_before_publication")
                || publisher_journal.file.identity.uid != owner
                || publisher_journal.file.identity.mode != 0o600
                || publisher_journal.file.identity.size > 1024 * 1024
                || Path::new(&publisher_journal.file.path) != Path::new(directory).join(
                    format!(".taira-native-nginx-apply-{intended_operation_id}.receipt.ndjson"))
            {
                return Err(eyre!("terminal vacancy changes its actual requested publisher journal or phase"));
            }
        }
        // The maintained native nginx helper validates every remaining field and
        // retains its exact owner journal/publication/backup before any effect.
        Ok(())
    }
}

/// Native progress is durable before an effect and never certifies global readiness alone.
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub(super) struct NativeEdgeProgressV1 {
    pub(super) schema: String,
    pub(super) operation_id: String,
    pub(super) inventory_sha256: String,
    pub(super) authorization_sha256: String,
    pub(super) authorization_nonce: String,
    pub(super) host_pair_sha256: String,
    pub(super) host_identity_sha256: String,
    pub(super) custody_root: String,
    pub(super) sequence: u64,
    #[norito(required)]
    pub(super) predecessor_sha256: Option<String>,
    pub(super) request_sha256: String,
    #[norito(required)]
    pub(super) publication_operation_id: Option<String>,
    pub(super) status: String,
    #[norito(required)]
    pub(super) checkpoint_sha256: Option<String>,
    #[norito(required)]
    pub(super) completion_receipt_sha256: Option<String>,
}

impl NativeEdgeProgressV1 {
    pub(super) fn validate(
        &self,
        inventory: &InventoryV1,
        inventory_sha256: &str,
        authorization_sha256: &str,
    ) -> Result<()> {
        validate_operation_binding(
            &self.operation_id,
            &self.inventory_sha256,
            &self.authorization_sha256,
            &self.authorization_nonce,
            &self.host_pair_sha256,
            &self.host_identity_sha256,
            &self.custody_root,
            inventory,
            inventory_sha256,
            authorization_sha256,
        )?;
        if self.schema != PROGRESS_SCHEMA
            || self.sequence == 0
            || (self.sequence == 1) != self.predecessor_sha256.is_none()
            || !matches!(
                self.status.as_str(),
                "admitted"
                    | "staged"
                    | "cutover_requested"
                    | "awaiting_readiness"
                    | "edge_ready_unqualified"
                    | "sealing"
                    | "sealed"
                    | "cleanup_requested"
                    | "cleaned"
                    | "rollback_requested"
                    | "rolled_back"
                    | "recovery_pending"
            )
        {
            return Err(eyre!(
                "native edge progress is outside its closed phase/sequence contract"
            ));
        }
        validate_lower_hex("native request", &self.request_sha256, 64)?;
        for value in [
            &self.predecessor_sha256,
            &self.checkpoint_sha256,
            &self.completion_receipt_sha256,
        ]
        .into_iter()
        .flatten()
        {
            validate_lower_hex("native progress digest", value, 64)?;
        }
        if let Some(operation) = &self.publication_operation_id {
            validate_lower_hex("native publication operation", operation, 32)?;
        }
        self.digest()?;
        Ok(())
    }

    pub(super) fn digest(&self) -> Result<String> {
        let body = json::to_json(self)?;
        if body.len() > MAX_PROGRESS_BYTES {
            return Err(eyre!("native edge progress exceeds its finite wire bound"));
        }
        Ok(sha256_hex(body.as_bytes()))
    }
}

/// Irreversible native fence admitted from the complete signed global proof chain.
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub(super) struct NativeGlobalProofFenceV1 {
    pub(super) schema: String,
    pub(super) operation_id: String,
    pub(super) inventory_sha256: String,
    pub(super) authorization_sha256: String,
    pub(super) authorization_nonce: String,
    pub(super) host_pair_sha256: String,
    pub(super) host_identity_sha256: String,
    pub(super) custody_root: String,
    pub(super) checkpoint_sha256: Vec<String>,
    pub(super) final_checkpoint_sha256: String,
    pub(super) progress_predecessor_sha256: String,
    pub(super) publication_operation_id: String,
}

impl NativeGlobalProofFenceV1 {
    pub(super) fn validate(
        &self,
        inventory: &InventoryV1,
        inventory_sha256: &str,
        authorization_sha256: &str,
        execution_expires_at_unix_ms: u64,
        checkpoints: &[SignedHostPhaseV1],
        predecessor: &NativeEdgeProgressV1,
    ) -> Result<()> {
        validate_operation_binding(
            &self.operation_id,
            &self.inventory_sha256,
            &self.authorization_sha256,
            &self.authorization_nonce,
            &self.host_pair_sha256,
            &self.host_identity_sha256,
            &self.custody_root,
            inventory,
            inventory_sha256,
            authorization_sha256,
        )?;
        predecessor.validate(inventory, inventory_sha256, authorization_sha256)?;
        host_pair::verify_checkpoint_chain(
            checkpoints,
            inventory,
            inventory_sha256,
            authorization_sha256,
            execution_expires_at_unix_ms,
            host_pair::HostPhaseV1::DeploymentProven,
        )?;
        let expected = checkpoints
            .iter()
            .map(SignedHostPhaseV1::digest)
            .collect::<Result<Vec<_>>>()?;
        if self.schema != FENCE_SCHEMA
            || self.checkpoint_sha256 != expected
            || self.checkpoint_sha256.last() != Some(&self.final_checkpoint_sha256)
            || self.progress_predecessor_sha256 != predecessor.digest()?
            || predecessor.operation_id != self.operation_id
            || predecessor.publication_operation_id.as_ref() != Some(&self.publication_operation_id)
            || predecessor.status != "edge_ready_unqualified"
            || predecessor.checkpoint_sha256.as_ref() != self.checkpoint_sha256.get(1)
        {
            return Err(eyre!(
                "native global proof fence does not join the exact ready owner and signed phase chain"
            ));
        }
        validate_lower_hex(
            "native fenced publication",
            &self.publication_operation_id,
            32,
        )
    }
}

fn validate_operation_binding(
    operation_id: &str,
    admitted_inventory: &str,
    admitted_authorization: &str,
    nonce: &str,
    hosts_sha: &str,
    identity: &str,
    custody_root: &str,
    inventory: &InventoryV1,
    inventory_sha256: &str,
    authorization_sha256: &str,
) -> Result<()> {
    validate_lower_hex("native reset operation", operation_id, 32)?;
    validate_lower_hex("native inventory", admitted_inventory, 64)?;
    validate_lower_hex("native authorization", admitted_authorization, 64)?;
    if admitted_inventory != inventory_sha256
        || admitted_authorization != authorization_sha256
        || nonce != inventory.authorization_nonce
        || hosts_sha != inventory.hosts.digest()?
        || identity != inventory.hosts.native_edge.endpoint.host_identity_sha256
        || custody_root != inventory.hosts.native_edge.custody_root
    {
        return Err(eyre!(
            "native operation belongs to another host, inventory or authorization"
        ));
    }
    Ok(())
}

/// Retained public bytes; the numeric descriptor is meaningful only to a direct native child.
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub(super) struct RetainedPublicRefV1 {
    pub(super) fd: u32,
    pub(super) reference: NativePublicFileV1,
}

/// The native Rust parent whose independently provisioned guard pins the executable.
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub(super) struct NativeParentV1 {
    pub(super) pid: u32,
    pub(super) uid: u32,
    pub(super) started: String,
    pub(super) executable: RetainedPublicRefV1,
}

/// Independently held per-authorization lock inherited by the direct native helper.
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub(super) struct NativeRetainedLockV1 {
    pub(super) fd: u32,
    pub(super) file: NativeObservedFileV1,
}

/// Rollback checks absence beneath a retained directory; terminal actions retain the fence.
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(tag = "kind", content = "value", deny_unknown_fields)]
pub(super) enum NativeFenceCustodyV1 {
    #[norito(rename = "absent")]
    Absent {
        directory_fd: u32,
        directory: NativeObservedFileV1,
        basename: String,
    },
    #[norito(rename = "present")]
    Present { reference: RetainedPublicRefV1 },
}

/// Closed helper admission, created only after native Rust signature and phase verification.
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub(super) struct NativeCompletionAdmissionV1 {
    pub(super) schema: String,
    pub(super) action: String,
    pub(super) operation_id: String,
    pub(super) inventory_sha256: String,
    pub(super) authorization_sha256: String,
    pub(super) authorization_nonce: String,
    pub(super) host_pair_sha256: String,
    pub(super) host_identity_sha256: String,
    pub(super) custody_root: String,
    pub(super) helper_source_closure_sha256: String,
    pub(super) parent: NativeParentV1,
    pub(super) lock: NativeRetainedLockV1,
    pub(super) guard: RetainedPublicRefV1,
    pub(super) inventory: RetainedPublicRefV1,
    pub(super) authorization: RetainedPublicRefV1,
    pub(super) progress: RetainedPublicRefV1,
    pub(super) checkpoints: Vec<RetainedPublicRefV1>,
    pub(super) fence: NativeFenceCustodyV1,
    pub(super) plan: RetainedPublicRefV1,
}

/// Effect receipt joined to the exact pre-effect progress and native journal.
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub(super) struct NativeCompletionReceiptV1 {
    pub(super) schema: String,
    pub(super) action: String,
    pub(super) operation_id: String,
    pub(super) inventory_sha256: String,
    pub(super) authorization_sha256: String,
    pub(super) authorization_nonce: String,
    pub(super) host_pair_sha256: String,
    pub(super) host_identity_sha256: String,
    pub(super) custody_root: String,
    pub(super) progress_before_sha256: String,
    #[norito(required)]
    pub(super) global_proof_sha256: Option<String>,
    pub(super) publication_operation_id: String,
    #[norito(required)]
    pub(super) restored_owned_publication: Option<host_pair::NativeOwnedPublicationV1>,
    pub(super) completion_journal: NativePublicFileV1,
    pub(super) status: String,
    #[norito(required)]
    pub(super) error_code: Option<String>,
}

/// Exact numeric native stat form. Never pass these integers through a JS number.
#[cfg(unix)]
pub(super) fn native_file_identity(
    metadata: &std::fs::Metadata,
) -> Result<host_pair::NativeFileIdentityV1> {
    let nanos = |seconds: i64, fractional: i64| -> Result<u64> {
        u64::try_from(seconds)?
            .checked_mul(1_000_000_000)
            .and_then(|value| value.checked_add(u64::try_from(fractional).ok()?))
            .ok_or_else(|| eyre!("native file timestamp exceeds the exact numeric wire"))
    };
    Ok(host_pair::NativeFileIdentityV1 {
        device: metadata.dev(),
        inode: metadata.ino(),
        uid: metadata.uid(),
        gid: metadata.gid(),
        mode: u16::try_from(metadata.mode() & 0o7777)?,
        links: metadata.nlink(),
        size: metadata.len(),
        mtime_ns: nanos(metadata.mtime(), metadata.mtime_nsec())?,
        ctime_ns: nanos(metadata.ctime(), metadata.ctime_nsec())?,
    })
}

/// Read only an independently declared public record, retaining exact FD/path/ancestor custody.
#[cfg(unix)]
pub(super) fn read_native_public(
    reference: &NativePublicFileV1,
    owner_uid: u32,
    maximum: usize,
) -> Result<Vec<u8>> {
    reference.validate_public(owner_uid)?;
    if reference.file.identity.size > u64::try_from(maximum)? {
        return Err(eyre!(
            "native public record exceeds the selected phase bound"
        ));
    }
    let mut retained = iroha_fs::RetainedFile::open_private(&reference.file.path)?;
    if native_file_identity(&retained.file().metadata()?)? != reference.file.identity {
        return Err(eyre!(
            "native public record metadata differs from its admitted reference"
        ));
    }
    retained.revalidate()?;
    let mut bytes = Vec::with_capacity(usize::try_from(reference.file.identity.size)?);
    retained
        .file_mut()
        .take(
            u64::try_from(maximum)?
                .checked_add(1)
                .ok_or_else(|| eyre!("native public bound overflow"))?,
        )
        .read_to_end(&mut bytes)?;
    if bytes.len() > maximum
        || u64::try_from(bytes.len())? != reference.file.identity.size
        || sha256_hex(&bytes) != reference.sha256
        || native_file_identity(&retained.file().metadata()?)? != reference.file.identity
    {
        return Err(eyre!(
            "native public record changed during its bounded admitted read"
        ));
    }
    retained.revalidate()?;
    Ok(bytes)
}

/// Capture terminal state through the same typed receiver protocol and signature verifier.
/// Publication-only incumbents do not possess distributed terminal proof.
#[cfg(unix)]
pub(super) fn validate_terminal_provenance(
    provenance: &host_pair::NativeEdgeCompletionProvenanceV1,
    owned_publication: Option<&host_pair::NativeOwnedPublicationV1>,
    inventory: &InventoryV1,
    inventory_sha256: &str,
    authorization_sha256: &str,
    execution_expires_at_unix_ms: u64,
) -> Result<()> {
    let host_pair::NativeEdgeCompletionProvenanceV1::ResetTerminal {
        status,
        progress,
        completion_receipt,
        checkpoints,
        global_proof,
        global_proof_predecessor,
    } = provenance
    else {
        return Err(eyre!(
            "publication-only capture cannot certify a completed distributed reset"
        ));
    };
    let owner = inventory.hosts.native_edge.owner_uid;
    let terminal_root = Path::new(&inventory.hosts.native_edge.custody_root)
        .join("taira-edge/operations")
        .join(authorization_sha256);
    for reference in std::iter::once(progress)
        .chain(std::iter::once(completion_receipt))
        .chain(checkpoints.iter())
        .chain(global_proof.iter())
        .chain(global_proof_predecessor.iter())
    {
        if reference.file.identity.uid != owner
            || reference.file.identity.mode != 0o600
            || !Path::new(&reference.file.path).starts_with(&terminal_root)
        {
            return Err(eyre!(
                "terminal native proof escaped the retained per-authorization owner"
            ));
        }
    }
    if Path::new(&progress.file.path) != terminal_root.join("progress.json") {
        return Err(eyre!(
            "native terminal progress has another anchored pathname"
        ));
    }
    let terminal: NativeEdgeProgressV1 =
        json::from_slice(&read_native_public(progress, owner, MAX_PROGRESS_BYTES)?)?;
    terminal.validate(inventory, inventory_sha256, authorization_sha256)?;
    if terminal.digest()? != progress.sha256 {
        return Err(eyre!(
            "native terminal progress is not its exact canonical retained bytes"
        ));
    }
    let receipt: NativeCompletionReceiptV1 =
        json::from_slice(&read_native_public(completion_receipt, owner, 64 * 1024)?)?;
    validate_operation_binding(
        &receipt.operation_id,
        &receipt.inventory_sha256,
        &receipt.authorization_sha256,
        &receipt.authorization_nonce,
        &receipt.host_pair_sha256,
        &receipt.host_identity_sha256,
        &receipt.custody_root,
        inventory,
        inventory_sha256,
        authorization_sha256,
    )?;
    let action = match status.as_str() {
        "sealed" => "seal",
        "cleaned" => "cleanup",
        "rolled_back" => "rollback",
        _ => {
            return Err(eyre!(
                "native terminal provenance is not a completed effect"
            ));
        }
    };
    if receipt.schema != "iroha.taira.public-reset.native-edge-completion-receipt.v1"
        || receipt.action != action
        || receipt.status != *status
        || terminal.status != *status
        || receipt.error_code.is_some()
        || terminal.operation_id != receipt.operation_id
        || terminal.publication_operation_id.as_ref() != Some(&receipt.publication_operation_id)
        || terminal.predecessor_sha256.as_ref() != Some(&receipt.progress_before_sha256)
        || terminal.completion_receipt_sha256.as_ref() != Some(&completion_receipt.sha256)
        || Path::new(&receipt.completion_journal.file.path)
            != terminal_root.join("native-completion.ndjson")
    {
        return Err(eyre!(
            "native completion receipt does not join the exact terminal progress/publication owner"
        ));
    }
    read_native_public(&receipt.completion_journal, owner, 1024 * 1024)?;
    let mut chain = Vec::with_capacity(checkpoints.len());
    for (index, reference) in checkpoints.iter().enumerate() {
        if Path::new(&reference.file.path)
            != terminal_root.join(format!("checkpoint-{}.json", index + 1))
        {
            return Err(eyre!(
                "native terminal checkpoint has another phase pathname"
            ));
        }
        let checkpoint: SignedHostPhaseV1 = json::from_slice(&read_native_public(
            reference,
            owner,
            host_pair::MAX_CHECKPOINT_BYTES,
        )?)?;
        if checkpoint.digest()? != reference.sha256 {
            return Err(eyre!(
                "native terminal checkpoint is not the exact canonical signed body"
            ));
        }
        chain.push(checkpoint);
    }
    if status == "rolled_back" {
        match (receipt.restored_owned_publication.as_ref(), owned_publication) {
            (Some(restored), Some(owned)) => {
                if inventory.edge.is_vacant()
                    || restored != owned
                    || restored.operation_id == receipt.publication_operation_id
                {
                    return Err(eyre!(
                        "native rollback terminal proof names another restored predecessor"
                    ));
                }
                read_native_public(&restored.journal, owner, 1024 * 1024)?;
                read_native_public(&restored.publication, owner, 1024 * 1024)?;
            }
            (None, None) if inventory.edge.is_vacant() => {
                // The signed recapture independently observes actual vacancy
                // through the maintained native inspector before this join.
                // Absence never substitutes for an admitted prior publisher.
            }
            _ => {
                return Err(eyre!(
                    "native rollback terminal proof changes its original vacant or admitted owner"
                ));
            }
        }
        if global_proof.is_some()
            || global_proof_predecessor.is_some()
            || receipt.global_proof_sha256.is_some()
            || chain.len() > 2
        {
            return Err(eyre!(
                "native rollback claims a forbidden global proof fence"
            ));
        }
        if let Some(checkpoint) = chain.last() {
            host_pair::verify_checkpoint_chain(
                &chain,
                inventory,
                inventory_sha256,
                authorization_sha256,
                execution_expires_at_unix_ms,
                checkpoint.claims.phase,
            )?;
        }
        // Absence is independently checked under retained native directory custody.
        let directory = iroha_fs::PrivateDirectory::open(&terminal_root)?;
        if directory
            .entries(128)?
            .iter()
            .any(|name| name == "global-proof.json")
        {
            return Err(eyre!(
                "native rollback terminal proof retains an irreversible global fence"
            ));
        }
        directory.revalidate()?;
    } else {
        let owned_publication = owned_publication
            .ok_or_else(|| eyre!("native seal lacks its exact current publication owner"))?;
        if receipt.restored_owned_publication.is_some()
            || receipt.publication_operation_id != owned_publication.operation_id
        {
            return Err(eyre!("native seal changed its current publication owner"));
        }
        let reference = global_proof
            .as_ref()
            .ok_or_else(|| eyre!("native seal lacks its immutable global proof fence"))?;
        let ready = global_proof_predecessor
            .as_ref()
            .ok_or_else(|| eyre!("native seal lacks its immutable pre-proof ready snapshot"))?;
        if Path::new(&reference.file.path) != terminal_root.join("global-proof.json")
            || Path::new(&ready.file.path) != terminal_root.join("global-proof-predecessor.json")
            || receipt.global_proof_sha256.as_ref() != Some(&reference.sha256)
        {
            return Err(eyre!(
                "native seal fence has another anchored owner or receipt digest"
            ));
        }
        let fence: NativeGlobalProofFenceV1 =
            json::from_slice(&read_native_public(reference, owner, MAX_PROGRESS_BYTES)?)?;
        let ready: NativeEdgeProgressV1 =
            json::from_slice(&read_native_public(ready, owner, MAX_PROGRESS_BYTES)?)?;
        if ready.digest()?
            != global_proof_predecessor
                .as_ref()
                .expect("required predecessor above")
                .sha256
            || ready.digest()? != fence.progress_predecessor_sha256
        {
            return Err(eyre!(
                "native global proof predecessor is not the exact immutable canonical snapshot"
            ));
        }
        fence.validate(
            inventory,
            inventory_sha256,
            authorization_sha256,
            execution_expires_at_unix_ms,
            &chain,
            &ready,
        )?;
        if fence.operation_id != terminal.operation_id
            || fence.publication_operation_id != owned_publication.operation_id
            || terminal.checkpoint_sha256.as_ref() != Some(&fence.final_checkpoint_sha256)
        {
            return Err(eyre!(
                "native terminal progress changes its fenced publication or final checkpoint"
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
pub(super) fn fixture_capability(
    hosts: &host_pair::ResetHostPairV1,
    release: EdgeAdmittedReleaseV1,
    nonce: &str,
    genesis: &str,
) -> NativeEdgeCapabilityV1 {
    let incumbent = host_pair::fixture_native_edge_capture(
        hosts,
        release,
        &"3".repeat(64),
        &"4".repeat(64),
        nonce,
        genesis,
    );
    let mut apply = incumbent.claims.forwarding_plan.clone();
    apply.file.path = format!(
        "{}/native-nginx-apply-plan.json",
        hosts.native_edge.custody_root
    );
    apply.file.identity.inode = 10;
    let mut incumbent_request = apply.clone();
    incumbent_request.file.path = format!(
        "{}/native-nginx-incumbent-request.json",
        hosts.native_edge.custody_root
    );
    incumbent_request.file.identity.inode = 11;
    NativeEdgeCapabilityV1 {
        schema: "iroha.taira.public-reset.native-edge-capability.v1".into(),
        captured_hosts: hosts.clone(),
        host_pair_sha256: hosts.digest().unwrap(),
        helper_source_closure_sha256: host_pair::helper_source_closure_sha256(),
        forwarding_plan: incumbent.claims.forwarding_plan.clone(),
        forwarding_identity_receipt: incumbent.claims.forwarding_identity_receipt.clone(),
        incumbent,
        incumbent_nginx_request: Some(incumbent_request),
        nginx_apply_plan: apply,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vacant_completion(operation: &str) -> NativeNginxCompletionPlanV1 {
        NativeNginxCompletionPlanV1 {
            schema: "iroha.taira.public-reset.native-nginx-completion-plan.v1".into(),
            nginx: norito::json!({
                "schema": "iroha.taira.native-nginx-apply.plan.v1",
                "host_kind": "macos",
                "provider": "macstadium-dublin",
                "operation_id": (operation.to_owned()),
                "publication": { "kind": "create" }
            }),
            publication_effect: NativePublicationEffectV1::VacantNotRequested {
                intended_operation_id: operation.to_owned(),
            },
            completion_journal_basename: "native-completion.ndjson".into(),
        }
    }

    #[test]
    fn vacant_no_effect_completion_requires_exact_create_and_intended_operation() {
        let intended = "a".repeat(32);
        let plan = vacant_completion(&intended);
        plan.validate(&intended).unwrap();
        assert!(plan.validate(&"b".repeat(32)).is_err());
        for publication in [
            norito::json!({ "kind": "create", "prior": null }),
            norito::json!({ "kind": "replace" }),
            norito::json!({ "kind": "reconcile" }),
        ] {
            let mut changed = plan.clone();
            changed.nginx.as_object_mut().unwrap().insert("publication".into(), publication);
            assert!(changed.validate(&intended).is_err());
        }
        let mut changed = plan;
        changed.nginx.as_object_mut().unwrap()
            .insert("operation_id".into(), Value::String("b".repeat(32)));
        assert!(changed.validate(&intended).is_err());
    }

    #[test]
    fn vacant_no_effect_publication_wire_has_no_predecessor_or_default() {
        let intended = "a".repeat(32);
        let wire = json::to_json(&NativePublicationEffectV1::VacantNotRequested {
            intended_operation_id: intended.clone(),
        }).unwrap();
        let parsed: NativePublicationEffectV1 = json::from_str(&wire).unwrap();
        assert!(matches!(parsed, NativePublicationEffectV1::VacantNotRequested {
            intended_operation_id,
        } if intended_operation_id == intended));
        for invalid in [
            r#"{"kind":"vacant_not_requested","value":{}}"#,
            r#"{"kind":"vacant_not_requested","value":{"intended_operation_id":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","incumbent":null}}"#,
            r#"{"kind":"vacant_not_requested"}"#,
            r#"{"kind":"legacy","value":null}"#,
        ] {
            assert!(json::from_str::<NativePublicationEffectV1>(invalid).is_err());
        }
    }

    fn terminal_vacant_completion(operation: &str, phase: &str) -> NativeNginxCompletionPlanV1 {
        let mut plan = vacant_completion(operation);
        plan.nginx.as_object_mut().unwrap().insert("native".into(), norito::json!({
            "owner_uid": 7, "directory": {"path": "/native/nginx"}
        }));
        plan.publication_effect = NativePublicationEffectV1::VacantPublisherTerminal {
            intended_operation_id: operation.to_owned(), terminal_phase: phase.to_owned(),
            publisher_journal: NativePublicFileV1 {
                file: NativeObservedFileV1 {
                    path: format!("/native/nginx/.taira-native-nginx-apply-{operation}.receipt.ndjson"),
                    identity: host_pair::NativeFileIdentityV1 {
                        device: 1, inode: 2, uid: 7, gid: 7, mode: 0o600, links: 1,
                        size: 8, mtime_ns: 9, ctime_ns: 10,
                    },
                },
                sha256: "c".repeat(64),
            },
        };
        plan
    }

    #[test]
    fn terminal_vacancy_requires_exact_requested_journal_and_closed_phase() {
        let intended = "a".repeat(32);
        for phase in ["rolled_back_unqualified", "refused_before_publication"] {
            let plan = terminal_vacant_completion(&intended, phase);
            plan.validate(&intended).unwrap();
            assert!(plan.validate(&"b".repeat(32)).is_err());
        }
        for phase in ["prepared", "rollback_ambiguous", "awaiting_readiness", "vacant"] {
            assert!(terminal_vacant_completion(&intended, phase).validate(&intended).is_err());
        }
        let mut changed = terminal_vacant_completion(&intended, "rolled_back_unqualified");
        if let NativePublicationEffectV1::VacantPublisherTerminal { publisher_journal, .. } = &mut changed.publication_effect {
            publisher_journal.file.path = "/native/nginx/foreign.receipt.ndjson".into();
        }
        assert!(changed.validate(&intended).is_err());
    }

    #[test]
    fn terminal_vacancy_wire_requires_actual_journal_and_phase_without_defaults() {
        let plan = terminal_vacant_completion(&"a".repeat(32), "refused_before_publication");
        let wire = json::to_json(&plan.publication_effect).unwrap();
        let parsed: NativePublicationEffectV1 = json::from_str(&wire).unwrap();
        assert!(matches!(parsed, NativePublicationEffectV1::VacantPublisherTerminal {
            terminal_phase, ..
        } if terminal_phase == "refused_before_publication"));
        for field in ["publisher_journal", "terminal_phase", "intended_operation_id"] {
            let mut value = json::to_value(&plan.publication_effect).unwrap();
            value.as_object_mut().unwrap().get_mut("value").unwrap().as_object_mut().unwrap().remove(field);
            assert!(json::from_str::<NativePublicationEffectV1>(&json::to_json(&value).unwrap()).is_err());
        }
    }

    #[test]
    fn native_capability_requires_explicit_incumbent_request_option() {
        let hosts = host_pair::fixture_pair();
        let release = EdgeAdmittedReleaseV1 {
            commit: "a".repeat(40),
            release_root: format!("{}/edge/releases/{}", hosts.native_edge.custody_root, "a".repeat(40)),
            cli_sha256: "b".repeat(64),
            config_sha256: "c".repeat(64),
        };
        let capability = fixture_capability(&hosts, release, "d".repeat(32).as_str(), "fixture-genesis");
        let mut value = json::to_value(&capability).unwrap();
        value.as_object_mut().unwrap().remove("incumbent_nginx_request");
        assert!(json::from_str::<NativeEdgeCapabilityV1>(&json::to_json(&value).unwrap()).is_err());
        value.as_object_mut().unwrap().insert("incumbent_nginx_request".into(), Value::Null);
        assert!(json::from_str::<NativeEdgeCapabilityV1>(&json::to_json(&value).unwrap()).is_ok());
    }
}
