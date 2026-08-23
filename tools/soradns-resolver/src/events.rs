use crate::state::{BundleDiff, BundleSnapshot, ExpiryDiff, ResolverDiff, ResolverInvalidation};
use eyre::{Result, bail};
use norito_derive::{JsonDeserialize, JsonSerialize, NoritoSerialize};
use std::path::PathBuf;
use tokio::sync::broadcast;
use tracing::{info, warn};
/// Resolver event payload emitted via logs and streaming interfaces.
#[derive(Clone, Debug, NoritoSerialize, JsonSerialize, JsonDeserialize)]
#[norito(tag = "event", content = "value")]
pub enum ResolverEvent {
    #[norito(rename = "bundle.expired")]
    BundleExpired {
        namehash: String,
        snapshot: BundleSnapshot,
    },
    BundleAdded {
        namehash: String,
        snapshot: BundleSnapshot,
    },
    BundleUpdated {
        namehash: String,
        previous: BundleSnapshot,
        current: BundleSnapshot,
    },
    BundleReorged {
        namehash: String,
        previous: BundleSnapshot,
        current: BundleSnapshot,
    },
    BundleRemoved {
        namehash: String,
        snapshot: BundleSnapshot,
    },
    #[norito(rename = "resolver.invalidate")]
    ResolverInvalidated {
        resolver_id: String,
        reason: String,
    },
    ResolverAdded {
        resolver_id: String,
    },
    ResolverUpdated {
        resolver_id: String,
    },
    ResolverRemoved {
        resolver_id: String,
    },
}
#[derive(Clone, Debug, NoritoSerialize, JsonSerialize, JsonDeserialize)]
pub struct ResolverEventLog {
    pub timestamp: i64,
    pub resolver_id: String,
    pub event: ResolverEvent,
}
/// Event emitter backed by a broadcast channel and optional durable log.
#[derive(Clone)]
pub struct EventEmitter {
    tx: broadcast::Sender<ResolverEvent>,
}
impl EventEmitter {
    pub fn new(_resolver_id: impl Into<String>, log_path: Option<PathBuf>) -> Result<Self> {
        if log_path.is_some() {
            bail!(
                "event_log_path is disabled in v1; use the bounded loopback event stream or process logging"
            );
        }
        let (tx, _rx) = broadcast::channel(128);
        Ok(Self { tx })
    }
    /// Subscribe to resolver events.
    pub fn subscribe(&self) -> broadcast::Receiver<ResolverEvent> {
        self.tx.subscribe()
    }
    /// Emit events for bundle changes, logging alongside the broadcast.
    pub fn emit_bundle_diff(&self, diff: &BundleDiff) {
        for (namehash, snapshot) in &diff.added {
            let event = ResolverEvent::BundleAdded {
                namehash: namehash.clone(),
                snapshot: snapshot.clone(),
            };
            self.emit(event, "bundle added", namehash);
        }
        for (namehash, previous, current) in &diff.updated {
            let event = ResolverEvent::BundleUpdated {
                namehash: namehash.clone(),
                previous: previous.clone(),
                current: current.clone(),
            };
            self.emit(event, "bundle version advanced", namehash);
        }
        for (namehash, previous, current) in &diff.reorged {
            let event = ResolverEvent::BundleReorged {
                namehash: namehash.clone(),
                previous: previous.clone(),
                current: current.clone(),
            };
            self.emit_warn(event, "bundle version regressed; reorg detected", namehash);
        }
        for (namehash, snapshot) in &diff.removed {
            let event = ResolverEvent::BundleRemoved {
                namehash: namehash.clone(),
                snapshot: snapshot.clone(),
            };
            self.emit(event, "bundle removed", namehash);
        }
    }
    /// Emit events for resolver advert changes, logging the transition.
    pub fn emit_resolver_diff(&self, diff: &ResolverDiff) {
        for resolver in &diff.added {
            let event = ResolverEvent::ResolverAdded {
                resolver_id: resolver.clone(),
            };
            self.emit(event, "resolver advert added", resolver);
        }
        for resolver in &diff.updated {
            let event = ResolverEvent::ResolverUpdated {
                resolver_id: resolver.clone(),
            };
            self.emit(event, "resolver advert updated", resolver);
        }
        for resolver in &diff.removed {
            let event = ResolverEvent::ResolverRemoved {
                resolver_id: resolver.clone(),
            };
            self.emit(event, "resolver advert removed", resolver);
        }
    }
    /// Emit invalidation events for expired bundles and resolver attestations.
    pub fn emit_expirations(&self, diff: &ExpiryDiff) {
        for (namehash, snapshot) in &diff.expired_bundles {
            let event = ResolverEvent::BundleExpired {
                namehash: namehash.clone(),
                snapshot: snapshot.clone(),
            };
            self.emit_warn(event, "bundle expired; pruning entry", namehash);
        }
        for ResolverInvalidation {
            resolver_id,
            reason,
        } in &diff.expired_resolvers
        {
            let event = ResolverEvent::ResolverInvalidated {
                resolver_id: resolver_id.clone(),
                reason: reason.clone(),
            };
            self.emit_warn(
                event,
                "resolver attestation invalidated; pruning entry",
                resolver_id,
            );
        }
    }
    fn emit(&self, event: ResolverEvent, log_message: &str, label: &str) {
        info!(event = %log_message, subject = %label);
        let _ = self.tx.send(event);
    }
    fn emit_warn(&self, event: ResolverEvent, log_message: &str, label: &str) {
        warn!(event = %log_message, subject = %label);
        let _ = self.tx.send(event);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{
        BundleDiff, BundleSnapshot, ExpiryDiff, ResolverDiff, ResolverInvalidation,
    };
    #[test]
    fn file_event_logging_is_disabled_for_v1() {
        let result = EventEmitter::new("resolver.test", Some(PathBuf::from("resolver.log")));
        let error = match result {
            Ok(_) => panic!("file event logging must fail closed"),
            Err(error) => error,
        };
        assert!(error.to_string().contains("disabled in v1"));
    }
    #[tokio::test]
    async fn subscriber_receives_resolver_events() {
        let emitter = EventEmitter::new("resolver.test", None).expect("construct emitter");
        let mut subscriber = emitter.subscribe();
        let mut diff = ResolverDiff::default();
        diff.updated.push("resolver.test".to_string());
        emitter.emit_resolver_diff(&diff);
        assert!(matches!(
            subscriber.recv().await.expect("resolver event"),
            ResolverEvent::ResolverUpdated { resolver_id } if resolver_id == "resolver.test"
        ));
    }
    #[tokio::test]
    async fn subscriber_receives_expired_entries() {
        let emitter = EventEmitter::new("resolver.test", None).expect("construct emitter");
        let mut subscriber = emitter.subscribe();
        let mut diff = ExpiryDiff::default();
        diff.expired_bundles.push((
            "abcd".to_string(),
            BundleSnapshot {
                zone_version: 7,
                manifest_hash_hex: "aa".into(),
                policy_hash_hex: "bb".into(),
                car_root_cid: "cid".into(),
                freshness_issued_at: 10,
                freshness_expires_at: 20,
                freshness_signer: "signer".into(),
                freshness_signature_hex: "dead".into(),
            },
        ));
        diff.expired_resolvers.push(ResolverInvalidation {
            resolver_id: "deadbeef".into(),
            reason: "expired".into(),
        });
        emitter.emit_expirations(&diff);
        assert!(matches!(
            subscriber.recv().await.expect("bundle expiry event"),
            ResolverEvent::BundleExpired { namehash, .. } if namehash == "abcd"
        ));
        assert!(matches!(
            subscriber.recv().await.expect("resolver expiry event"),
            ResolverEvent::ResolverInvalidated { resolver_id, .. } if resolver_id == "deadbeef"
        ));
    }
    #[tokio::test]
    async fn subscriber_receives_bundle_event() {
        let emitter = EventEmitter::new("resolver.test", None).expect("construct emitter");
        let mut subscriber = emitter.subscribe();
        let mut diff = BundleDiff::default();
        diff.added.push((
            "zzzz".to_string(),
            BundleSnapshot {
                zone_version: 9,
                manifest_hash_hex: "aa".into(),
                policy_hash_hex: "bb".into(),
                car_root_cid: "cid".into(),
                freshness_issued_at: 10,
                freshness_expires_at: 30,
                freshness_signer: "council".into(),
                freshness_signature_hex: "ff".into(),
            },
        ));
        emitter.emit_bundle_diff(&diff);
        match subscriber.recv().await.expect("receive bundle event") {
            ResolverEvent::BundleAdded { namehash, snapshot } => {
                assert_eq!(namehash, "zzzz");
                assert_eq!(snapshot.zone_version, 9);
                assert_eq!(snapshot.car_root_cid, "cid");
            }
            other => panic!("unexpected event: {other:?}"),
        }
    }
}
