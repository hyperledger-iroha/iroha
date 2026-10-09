//! Only directly owned child handles may witness a generated gateway or undergo restart.

use super::*;
use crate::localnet::service_authorities::RetainedGatewayCompliancePlan;
use crate::managed::{
    gateway_compliance::LiveGatewayProcess,
    generated_service_runtime::{GeneratedServiceRuntime, GeneratedServiceRuntimeRevision},
    native_operation::{ManagedTransactionFinality, invalid},
    stream_token_custody::RetainedCustodyEnrollment,
};
use iroha_data_model::sorafs::capacity::ProviderId;
use std::sync::Mutex;

/// Exact original and opaque renderer revision retained for every owned launch.
pub(super) struct GeneratedLaunch {
    owner: Arc<GeneratedServiceRuntime>,
    revision: Arc<GeneratedServiceRuntimeRevision>,
    original: PreparedLocalnet,
    active: AtomicBool,
}
impl GeneratedLaunch {
    pub(super) fn new(
        owner: Arc<GeneratedServiceRuntime>,
        revision: GeneratedServiceRuntimeRevision,
        original: &PreparedLocalnet,
    ) -> Result<Arc<Self>> {
        owner.validate_for(original, &revision)?;
        Ok(Arc::new(Self {
            owner,
            revision: Arc::new(revision),
            original: original.clone(),
            active: AtomicBool::new(true),
        }))
    }

    fn validate(&self) -> Result<()> {
        self.require_active()?;
        self.owner.validate_for(&self.original, &self.revision)?;
        self.require_active()
    }

    fn require_active(&self) -> Result<()> {
        if !self.active.load(Ordering::Acquire) {
            return Err(invalid("the original generated launch is stopping"));
        }
        Ok(())
    }

    fn command(&self, daemon: &std::path::Path, index: usize) -> Result<Command> {
        self.validate()?;
        let peer = self.revision.peer(index)?;
        let mut command = Command::new(daemon);
        command
            .arg("--sora")
            .arg("--config")
            .arg(peer.path())
            .arg("--config-blake3")
            .arg(hex::encode(peer.blake3()))
            .current_dir(self.revision.cwd());
        Ok(command)
    }
}

#[derive(Default)]
pub(super) struct PeerProcesses {
    pub(super) children: Vec<Arc<Mutex<Child>>>,
    launch: Option<Arc<GeneratedLaunch>>,
}

impl PeerProcesses {
    #[cfg(test)]
    pub(super) fn from_children(children: Vec<Child>) -> Self {
        Self {
            children: children
                .into_iter()
                .map(|child| Arc::new(Mutex::new(child)))
                .collect(),
            launch: None,
        }
    }

    pub(super) fn generated(
        &self,
    ) -> Result<Option<(Arc<GeneratedServiceRuntime>, [OwnedGateway; 3])>> {
        self.launch
            .as_ref()
            .map(|launch| {
                self.gateways()
                    .map(|gateways| (Arc::clone(&launch.owner), gateways))
            })
            .transpose()
    }

    pub(super) fn start(
        &mut self,
        directory: &PrivateDirectory,
        retained: &RetainedLocalnet,
        ownership: &File,
        daemon: &super::super::program::NativeProgram,
        launch: Option<Arc<GeneratedLaunch>>,
    ) -> Result<()> {
        if !self.children.is_empty() || self.launch.is_some() {
            return Err(invalid("the owned generation must stop before restarting"));
        }
        if let Some(launch) = &launch {
            if launch.original != retained.prepared
                || !matches!(retained.root_kind, RootKind::Global)
            {
                return Err(invalid(
                    "generated launch differs from its original global generation",
                ));
            }
            launch.validate()?;
        }
        if daemon.path() != retained.daemon.path.as_path() {
            return Err(invalid("daemon differs from the retained runtime path"));
        }
        if daemon.pin()?.blake3 != retained.daemon.blake3 {
            return Err(invalid("daemon differs from the retained runtime contents"));
        }
        self.launch = launch;
        for (index, peer) in retained.prepared.peers.iter().enumerate() {
            let log = directory.open_append(&peer.log_name)?;
            let mut command = match &self.launch {
                Some(launch) => launch.command(&retained.daemon.path, index)?,
                None => daemon_command(
                    &retained.daemon.path,
                    &peer.config_path,
                    &retained.root_kind,
                )?,
            };
            command
                .stdin(Stdio::from(ownership.try_clone()?))
                .stdout(log.try_clone()?)
                .stderr(log);
            daemon.validate()?;
            self.children
                .push(Arc::new(Mutex::new(spawn_with_launch_fence(
                    directory,
                    index,
                    &mut command,
                    daemon,
                )?)));
        }
        if let Some(launch) = &self.launch {
            launch.validate()?;
        }
        Ok(())
    }

    /// Select only the exact original provider's directly owned child in this launch.
    pub(super) fn gateway(&self, provider: ProviderId) -> Result<OwnedGateway> {
        let launch = self
            .launch
            .as_ref()
            .ok_or_else(|| invalid("generated gateway is absent"))?;
        if self.children.len() != 4 {
            return Err(invalid("generated gateway has no complete owned committee"));
        }
        launch.validate()?;
        let plan = launch
            .original
            .provider_service_plan(provider)?
            .ok_or_else(|| invalid("original provider plan absent"))?;
        let gateway = OwnedGateway {
            child: self
                .children
                .get(plan.peer_index())
                .ok_or_else(|| invalid("original provider child absent"))?
                .clone(),
            launch: launch.clone(),
            provider,
        };
        gateway.require_running()?;
        Ok(gateway)
    }

    pub(super) fn gateways(&self) -> Result<[OwnedGateway; 3]> {
        let launch = self
            .launch
            .as_ref()
            .ok_or_else(|| invalid("generated launch absent"))?;
        let plans = launch
            .original
            .provider_service_plans()?
            .ok_or_else(|| invalid("original provider plans absent"))?;
        plans
            .iter()
            .map(|plan| self.gateway(plan.provider_id()))
            .collect::<Result<Vec<_>>>()?
            .try_into()
            .map_err(|_| invalid("owned gateway count differs"))
    }

    pub(super) fn any_exited(&mut self) -> Result<bool> {
        for child in &self.children {
            if child
                .lock()
                .map_err(|_| invalid("owned child lock failed"))?
                .try_wait()?
                .is_some()
            {
                return Ok(true);
            }
        }
        Ok(false)
    }

    pub(super) fn stop(&mut self) -> Result<()> {
        // Invalidate background guards before graceful shutdown begins, even if a child has
        // not exited yet. An already dispatched exact request retains its normal recovery.
        if let Some(launch) = &self.launch {
            launch.active.store(false, Ordering::Release);
        }
        // Only these directly owned, unreaped handles are eligible. A background gateway guard
        // borrows the same handles, so reaping immediately invalidates every old observation.
        for child in &self.children {
            let mut child = child
                .lock()
                .map_err(|_| invalid("owned child lock failed"))?;
            if child.try_wait()?.is_some() {
                continue;
            }
            #[cfg(unix)]
            if let Some(pid) = rustix::process::Pid::from_raw(child.id() as i32) {
                let _ = rustix::process::kill_process(pid, rustix::process::Signal::TERM);
            }
            #[cfg(not(unix))]
            child.kill()?;
        }
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let mut remaining = false;
            for child in &self.children {
                if child
                    .lock()
                    .map_err(|_| invalid("owned child lock failed"))?
                    .try_wait()?
                    .is_none()
                {
                    remaining = true;
                }
            }
            if !remaining || Instant::now() >= deadline {
                break;
            }
            thread::sleep(POLL);
        }
        for child in &self.children {
            let mut child = child
                .lock()
                .map_err(|_| invalid("owned child lock failed"))?;
            if child.try_wait()?.is_none() {
                child.kill()?;
            }
            child.wait()?;
        }
        self.children.clear();
        self.launch = None;
        Ok(())
    }
}

impl Drop for PeerProcesses {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

pub(super) struct OwnedGateway {
    child: Arc<Mutex<Child>>,
    launch: Arc<GeneratedLaunch>,
    provider: ProviderId,
}
impl OwnedGateway {
    pub(super) fn provider(&self) -> ProviderId {
        self.provider
    }
    pub(super) fn required_transactions(&self) -> Result<&[ManagedTransactionFinality]> {
        self.require_running()?;
        self.launch.validate()?;
        Ok(self.launch.revision.required_transactions())
    }
    pub(super) fn observation_floor(&self) -> Result<ManagedTransactionFinality> {
        self.require_running()?;
        self.launch.validate()?;
        self.launch.revision.observation_floor()
    }
    /// Borrow only the enrollment retained by this actual launch's opaque renderer revision.
    /// The requested transaction must be exactly the one that the worker confirmed on all peers.
    pub(super) fn selected_enrollment(
        &self,
        required: ManagedTransactionFinality,
    ) -> Result<&RetainedCustodyEnrollment> {
        self.require_running()?;
        self.launch.validate()?;
        if self.launch.revision.observation_floor()? != required {
            return Err(invalid(
                "owned runtime transaction differs from selected observation",
            ));
        }
        let selected = self
            .launch
            .revision
            .selected_enrollment(self.provider)?
            .ok_or_else(|| invalid("owned runtime has no retained enrollment"))?;
        self.require_running()?;
        Ok(selected)
    }

    /// Derive an exact successor of this actual launch, through its retained sole renderer.
    pub(super) fn prepare_successor(
        &self,
        sequence: u64,
        deadline: Instant,
    ) -> Result<(
        Arc<GeneratedServiceRuntime>,
        GeneratedServiceRuntimeRevision,
    )> {
        self.require_running()?;
        self.launch.validate()?;
        let revision = self.launch.owner.prepare_renewed_stream_tokens(
            &self.launch.revision,
            self.provider,
            sequence,
            deadline,
        )?;
        self.require_running()?;
        Ok((Arc::clone(&self.launch.owner), revision))
    }

    fn require_running(&self) -> Result<()> {
        self.launch.require_active()?;
        if self
            .child
            .lock()
            .map_err(|_| invalid("owned gateway lock failed"))?
            .try_wait()?
            .is_some()
        {
            return Err(invalid("the original generated gateway process exited"));
        }
        self.launch.require_active()
    }
}
impl LiveGatewayProcess for OwnedGateway {
    fn validate(
        &mut self,
        prepared: &PreparedLocalnet,
        plan: &RetainedGatewayCompliancePlan,
    ) -> Result<()> {
        self.require_running()?;
        if prepared != &self.launch.original {
            return Err(invalid(
                "the observed gateway belongs to another original generation",
            ));
        }
        self.launch.validate()?;
        let expected = prepared
            .gateway_compliance_plan(self.provider)?
            .ok_or_else(|| invalid("original gateway plan absent"))?;
        if expected.network_id() != plan.network_id()
            || expected.original_commitment() != plan.original_commitment()
            || expected.trust_policy() != plan.trust_policy()
            || expected.gateway_id() != plan.gateway_id()
        {
            return Err(invalid("the observed gateway compliance binding differs"));
        }
        self.require_running()
    }
}

#[cfg(test)]
mod tests;
