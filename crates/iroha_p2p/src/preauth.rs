//! Process-local resource bounds for accepted transports that have not authenticated yet.

use std::{
    collections::HashMap,
    future::Future,
    net::IpAddr,
    num::NonZeroUsize,
    sync::{Arc, Mutex},
    time::Duration,
};

use tokio::{
    sync::oneshot,
    time::{Instant, sleep_until, timeout_at},
};

/// Normalize equivalent socket source identities before applying resource bounds.
pub(crate) fn canonical_remote_ip(ip: IpAddr) -> IpAddr {
    match ip {
        IpAddr::V6(ip) => ip.to_ipv4_mapped().map_or(IpAddr::V6(ip), IpAddr::V4),
        IpAddr::V4(ip) => IpAddr::V4(ip),
    }
}

/// Shared concurrent source bound spanning every inbound transport listener.
pub(crate) struct PreauthSourceGate {
    max_per_ip: NonZeroUsize,
    counts: Mutex<HashMap<IpAddr, usize>>,
}

impl PreauthSourceGate {
    /// Construct one process-local gate from the validated network configuration.
    pub(crate) fn new(max_per_ip: NonZeroUsize) -> Self {
        Self {
            max_per_ip,
            counts: Mutex::new(HashMap::new()),
        }
    }

    /// Try to reserve one accepted, unauthenticated transport for `remote_ip`.
    pub(crate) fn try_acquire(self: &Arc<Self>, remote_ip: IpAddr) -> Option<PreauthSourcePermit> {
        let remote_ip = canonical_remote_ip(remote_ip);
        let mut counts = self
            .counts
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let count = counts.entry(remote_ip).or_default();
        if *count >= self.max_per_ip.get() {
            return None;
        }
        *count += 1;
        Some(PreauthSourcePermit {
            gate: Arc::clone(self),
            remote_ip,
        })
    }

    /// Return the configured concurrent bound for diagnostics.
    pub(crate) fn max_per_ip(&self) -> usize {
        self.max_per_ip.get()
    }

    #[cfg(test)]
    pub(crate) fn active_for(&self, remote_ip: IpAddr) -> usize {
        self.counts
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(&canonical_remote_ip(remote_ip))
            .copied()
            .unwrap_or_default()
    }
}

/// Exact RAII ownership of one accepted transport's source reservation.
pub(crate) struct PreauthSourcePermit {
    gate: Arc<PreauthSourceGate>,
    remote_ip: IpAddr,
}

impl Drop for PreauthSourcePermit {
    fn drop(&mut self) {
        let mut counts = self
            .gate
            .counts
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let remove = match counts.get_mut(&self.remote_ip) {
            Some(count) if *count > 1 => {
                *count -= 1;
                false
            }
            // Removing both `1` and an impossible `0` heals the table without
            // allowing teardown or a poisoned lock to panic.
            Some(_) => true,
            None => false,
        };
        if remove {
            counts.remove(&self.remote_ip);
        }
    }
}

/// Identifies which bound expired while running one pre-authentication stage.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DeadlineElapsed {
    /// The accepted transport exhausted its total authentication tenure.
    Absolute,
    /// The current stage exhausted its existing, shorter idle bound.
    Stage,
}

/// Immutable total deadline shared by every stage of one accepted transport.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PreauthDeadline(Instant);

impl PreauthDeadline {
    /// Create a deadline relative to now, returning `None` when it cannot be represented.
    pub(crate) fn from_now(timeout: Duration) -> Option<Self> {
        Instant::now().checked_add(timeout).map(Self)
    }

    /// Wait until the immutable absolute deadline expires.
    #[cfg(feature = "quic")]
    pub(crate) async fn wait(self) {
        sleep_until(self.0).await;
    }

    /// Return whether the immutable absolute deadline has elapsed.
    #[cfg(feature = "quic")]
    pub(crate) fn has_elapsed(self) -> bool {
        Instant::now() >= self.0
    }

    /// Run one stage under both this total deadline and an optional local stage timeout.
    pub(crate) async fn run<F>(
        self,
        stage_timeout: Option<Duration>,
        future: F,
    ) -> Result<F::Output, DeadlineElapsed>
    where
        F: Future,
    {
        let now = Instant::now();
        if now >= self.0 {
            return Err(DeadlineElapsed::Absolute);
        }

        let (effective_deadline, elapsed) = stage_timeout
            .and_then(|timeout| now.checked_add(timeout))
            .filter(|stage_deadline| *stage_deadline < self.0)
            .map_or((self.0, DeadlineElapsed::Absolute), |stage_deadline| {
                (stage_deadline, DeadlineElapsed::Stage)
            });

        match timeout_at(effective_deadline, future).await {
            Ok(output) if Instant::now() < effective_deadline => Ok(output),
            Ok(_) | Err(_) => Err(elapsed),
        }
    }

    /// Create the acknowledgement carried from the peer authentication boundary.
    pub(crate) fn completion_channel(
        self,
    ) -> (InboundAuthCompletion, oneshot::Receiver<AuthOutcome>) {
        let (sender, receiver) = oneshot::channel();
        (
            InboundAuthCompletion {
                deadline: self,
                sender,
            },
            receiver,
        )
    }

    /// Wait until authentication completes or the accepted transport exhausts its tenure.
    pub(crate) async fn wait_for_authentication(
        self,
        mut receiver: oneshot::Receiver<AuthOutcome>,
    ) -> bool {
        let timer = sleep_until(self.0);
        tokio::pin!(timer);
        let outcome = tokio::select! {
            biased;
            outcome = &mut receiver => outcome.ok(),
            () = &mut timer => receiver.try_recv().ok(),
        };
        matches!(
            outcome,
            Some(AuthOutcome::Authenticated { completed_at }) if completed_at < self.0
        )
    }
}

/// One-shot authority to acknowledge successful inbound application authentication.
pub(crate) struct InboundAuthCompletion {
    deadline: PreauthDeadline,
    sender: oneshot::Sender<AuthOutcome>,
}

impl InboundAuthCompletion {
    /// Return the total deadline governing the application handshake.
    pub(crate) fn deadline(&self) -> PreauthDeadline {
        self.deadline
    }

    /// Publish a timely authentication result while the listener still owns the receiver.
    pub(crate) fn complete(self) -> bool {
        let completed_at = Instant::now();
        completed_at < self.deadline.0
            && self
                .sender
                .send(AuthOutcome::Authenticated { completed_at })
                .is_ok()
    }
}

/// Timestamped result used to prevent a late sender from winning a timer scheduling race.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AuthOutcome {
    /// The signed peer identity and all configured capability gates passed.
    Authenticated {
        /// Local monotonic time at which authentication completed.
        completed_at: Instant,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_gate_is_shared_across_transports_and_canonical_addresses() {
        let gate = Arc::new(PreauthSourceGate::new(NonZeroUsize::new(2).unwrap()));
        let ipv4 = "192.0.2.10".parse().unwrap();
        let mapped = "::ffff:192.0.2.10".parse().unwrap();
        let other = "192.0.2.11".parse().unwrap();

        let tcp = gate.try_acquire(ipv4).expect("first transport");
        let quic = gate.try_acquire(mapped).expect("mapped transport");
        assert!(gate.try_acquire(ipv4).is_none());
        assert_eq!(gate.active_for(ipv4), 2);
        assert_eq!(gate.active_for(mapped), 2);
        assert!(gate.try_acquire(other).is_some());

        drop(tcp);
        assert_eq!(gate.active_for(ipv4), 1);
        drop(quic);
        assert_eq!(gate.active_for(ipv4), 0);
    }

    #[test]
    fn rejected_source_does_not_change_ownership_and_reconnects_release_exactly() {
        let gate = Arc::new(PreauthSourceGate::new(NonZeroUsize::new(1).unwrap()));
        let remote = "2001:db8::7".parse().unwrap();

        for _ in 0..3 {
            let permit = gate.try_acquire(remote).expect("source capacity");
            assert_eq!(gate.active_for(remote), 1);
            assert!(gate.try_acquire(remote).is_none());
            assert_eq!(gate.active_for(remote), 1);
            drop(permit);
            assert_eq!(gate.active_for(remote), 0);
        }
    }

    #[tokio::test(start_paused = true)]
    async fn stages_share_one_absolute_deadline() {
        let deadline = PreauthDeadline::from_now(Duration::from_millis(100)).unwrap();
        deadline
            .run(Some(Duration::from_millis(80)), async {
                tokio::time::sleep(Duration::from_millis(60)).await;
            })
            .await
            .unwrap();

        let result = deadline
            .run(Some(Duration::from_millis(80)), async {
                tokio::time::sleep(Duration::from_millis(50)).await;
            })
            .await;
        assert_eq!(result, Err(DeadlineElapsed::Absolute));
    }

    #[tokio::test(start_paused = true)]
    async fn ready_result_polled_after_deadline_is_rejected() {
        let deadline = PreauthDeadline::from_now(Duration::from_millis(10)).unwrap();
        let result = deadline
            .run(None, async {
                tokio::time::advance(Duration::from_millis(11)).await;
                7_u8
            })
            .await;
        assert_eq!(result, Err(DeadlineElapsed::Absolute));
    }

    #[tokio::test(start_paused = true)]
    async fn shorter_stage_timeout_remains_effective() {
        let deadline = PreauthDeadline::from_now(Duration::from_secs(1)).unwrap();
        let result = deadline
            .run(Some(Duration::from_millis(50)), async {
                tokio::time::sleep(Duration::from_millis(60)).await;
            })
            .await;
        assert_eq!(result, Err(DeadlineElapsed::Stage));
    }

    #[tokio::test(start_paused = true)]
    async fn timely_completion_survives_delayed_listener_poll() {
        let deadline = PreauthDeadline::from_now(Duration::from_millis(10)).unwrap();
        let (completion, receiver) = deadline.completion_channel();
        assert!(completion.complete());
        tokio::time::advance(Duration::from_millis(11)).await;
        assert!(deadline.wait_for_authentication(receiver).await);
    }

    #[tokio::test(start_paused = true)]
    async fn late_completion_cannot_enter_authenticated_state() {
        let deadline = PreauthDeadline::from_now(Duration::from_millis(10)).unwrap();
        let (completion, receiver) = deadline.completion_channel();
        tokio::time::advance(Duration::from_millis(10)).await;
        assert!(!completion.complete());
        assert!(!deadline.wait_for_authentication(receiver).await);
    }

    #[test]
    fn unrepresentable_deadline_is_rejected() {
        assert!(PreauthDeadline::from_now(Duration::MAX).is_none());
    }
}
