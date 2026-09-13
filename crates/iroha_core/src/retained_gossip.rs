//! Final gossip-consumer ownership of an authenticated P2P admission.
use iroha_p2p::peer::message::PeerMessageRetentionGuard;

/// One incoming gossip and its uncloneable transport byte/count owner.
/// Queue rejection drops both; acceptance retains both until the final physical
/// operation returns. This type does not authenticate a payload or mint credit.
pub struct RetainedGossip<T> {
    payload: T,
    retention: PeerMessageRetentionGuard,
}
impl<T> std::fmt::Debug for RetainedGossip<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RetainedGossip").finish_non_exhaustive()
    }
}
impl<T> RetainedGossip<T> {
    /// Move the existing P2P guard together with its payload into an actor.
    /// No guard clone, permit reacquisition, or unretained production overload.
    pub fn new(payload: T, retention: PeerMessageRetentionGuard) -> Self {
        Self { payload, retention }
    }
    pub(crate) fn payload(&self) -> &T {
        &self.payload
    }
    pub(crate) fn with_payload<R>(self, consume: impl FnOnce(T) -> R) -> R {
        let Self { payload, retention } = self;
        let result = consume(payload);
        // Keep the exact owner on this stack through every validation, persistence
        // and queue handoff in the callback, including unwinding on failure.
        drop(retention);
        result
    }
    /// Explicit synthetic construction only for unit tests of nontransport logic.
    #[cfg(test)]
    pub(crate) fn synthetic_for_test(payload: T) -> Self {
        let (_, _, _, _, guard) = test_message(false).0.into_parts();
        Self::new(payload, guard)
    }
    /// Real count-owner fixture shared by receiver tests, with no byte qualification.
    #[cfg(test)]
    pub(crate) fn with_count_for_test(
        payload: T,
    ) -> (Self, std::sync::Arc<tokio::sync::Semaphore>) {
        let (message, count) = test_message(true);
        let (_, _, _, _, guard) = message.into_parts();
        (Self::new(payload, guard), count)
    }
}
#[cfg(test)]
fn test_message(
    retained: bool,
) -> (
    iroha_p2p::peer::message::PeerMessage<crate::NetworkMessage>,
    std::sync::Arc<tokio::sync::Semaphore>,
) {
    use iroha_crypto::{Algorithm, KeyPair};
    use iroha_data_model::peer::Peer;
    let key = KeyPair::try_from_seed(vec![81; 32], Algorithm::BlsNormal).unwrap();
    let peer = Peer::new("127.0.0.1:9911".parse().unwrap(), key.public_key().clone());
    let mut message =
        iroha_p2p::peer::message::PeerMessage::new(peer, crate::NetworkMessage::Health, 1);
    let count = std::sync::Arc::new(tokio::sync::Semaphore::new(1));
    if retained {
        message.retain_authenticated_source_credit(count.clone().try_acquire_owned().unwrap());
    }
    (message, count)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn retained_count_survives_the_complete_consumer_and_releases_on_return() {
        let (message, count) = RetainedGossip::with_count_for_test(7);
        assert_eq!(count.available_permits(), 0);
        assert_eq!(
            message.with_payload(|value| {
                assert_eq!(count.available_permits(), 0);
                value + 1
            }),
            8
        );
        assert_eq!(count.available_permits(), 1);
    }
    #[test]
    fn retained_count_is_released_by_a_panicking_consumer() {
        let (message, count) = RetainedGossip::with_count_for_test(());
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| message.with_payload(
                |()| {
                    assert_eq!(count.available_permits(), 0);
                    panic!("deliberate consumer panic");
                }
            )))
            .is_err()
        );
        assert_eq!(count.available_permits(), 1);
    }
}
