//! Background send/queue helpers for consensus messages.

use iroha_logger::prelude::*;

use super::*;

#[cfg(feature = "telemetry")]
pub(super) fn dispatch_background_request(
    tx_opt: Option<&mpsc::SyncSender<BackgroundPost>>,
    request: BackgroundRequest,
    telemetry: &Telemetry,
) -> Result<(), Box<BackgroundRequest>> {
    let (kind, peer_for_metrics, post) = match request {
        BackgroundRequest::Post { peer, msg } => (
            "Post",
            Some(peer.clone()),
            BackgroundPost::Post {
                peer,
                msg,
                enqueued_at: Instant::now(),
            },
        ),
        BackgroundRequest::PostControlFlow { peer, frame } => (
            "PostControlFlow",
            Some(peer.clone()),
            BackgroundPost::PostControlFlow {
                peer,
                frame,
                enqueued_at: Instant::now(),
            },
        ),
        BackgroundRequest::Broadcast { msg } => (
            "Broadcast",
            None,
            BackgroundPost::Broadcast {
                msg,
                enqueued_at: Instant::now(),
            },
        ),
        BackgroundRequest::BroadcastControlFlow { frame } => (
            "BroadcastControlFlow",
            None,
            BackgroundPost::BroadcastControlFlow {
                frame,
                enqueued_at: Instant::now(),
            },
        ),
    };

    let Some(tx) = tx_opt else {
        trace!(kind, "dropping background request: no worker attached");
        return Err(Box::new(match post {
            BackgroundPost::Post { peer, msg, .. } => BackgroundRequest::Post { peer, msg },
            BackgroundPost::PostControlFlow { peer, frame, .. } => {
                BackgroundRequest::PostControlFlow { peer, frame }
            }
            BackgroundPost::Broadcast { msg, .. } => BackgroundRequest::Broadcast { msg },
            BackgroundPost::BroadcastControlFlow { frame, .. } => {
                BackgroundRequest::BroadcastControlFlow { frame }
            }
        }));
    };

    match tx.try_send(post) {
        Ok(()) => {
            telemetry.inc_bg_post_enqueued(kind);
            if let Some(peer) = peer_for_metrics.as_ref() {
                telemetry.inc_post_to_peer(peer);
                telemetry.inc_bg_post_queue_depth_for_peer(peer);
            }
            Ok(())
        }
        Err(mpsc::TrySendError::Full(post)) => {
            telemetry.inc_bg_post_overflow(kind);
            trace!(
                kind,
                "background post queue full; falling back to inline send"
            );
            Err(Box::new(match post {
                BackgroundPost::Post { peer, msg, .. } => BackgroundRequest::Post { peer, msg },
                BackgroundPost::PostControlFlow { peer, frame, .. } => {
                    BackgroundRequest::PostControlFlow { peer, frame }
                }
                BackgroundPost::Broadcast { msg, .. } => BackgroundRequest::Broadcast { msg },
                BackgroundPost::BroadcastControlFlow { frame, .. } => {
                    BackgroundRequest::BroadcastControlFlow { frame }
                }
            }))
        }
        Err(mpsc::TrySendError::Disconnected(post)) => {
            iroha_logger::warn!(kind, "background post channel disconnected");
            Err(Box::new(match post {
                BackgroundPost::Post { peer, msg, .. } => BackgroundRequest::Post { peer, msg },
                BackgroundPost::PostControlFlow { peer, frame, .. } => {
                    BackgroundRequest::PostControlFlow { peer, frame }
                }
                BackgroundPost::Broadcast { msg, .. } => BackgroundRequest::Broadcast { msg },
                BackgroundPost::BroadcastControlFlow { frame, .. } => {
                    BackgroundRequest::BroadcastControlFlow { frame }
                }
            }))
        }
    }
}

#[cfg(not(feature = "telemetry"))]
pub(super) fn dispatch_background_request(
    tx_opt: Option<&mpsc::SyncSender<BackgroundPost>>,
    request: BackgroundRequest,
) -> Result<(), Box<BackgroundRequest>> {
    let (kind, post) = match request {
        BackgroundRequest::Post { peer, msg } => (
            "Post",
            BackgroundPost::Post {
                peer,
                msg,
                enqueued_at: Instant::now(),
            },
        ),
        BackgroundRequest::PostControlFlow { peer, frame } => (
            "PostControlFlow",
            BackgroundPost::PostControlFlow {
                peer,
                frame,
                enqueued_at: Instant::now(),
            },
        ),
        BackgroundRequest::Broadcast { msg } => (
            "Broadcast",
            BackgroundPost::Broadcast {
                msg,
                enqueued_at: Instant::now(),
            },
        ),
        BackgroundRequest::BroadcastControlFlow { frame } => (
            "BroadcastControlFlow",
            BackgroundPost::BroadcastControlFlow {
                frame,
                enqueued_at: Instant::now(),
            },
        ),
    };

    let Some(tx) = tx_opt else {
        trace!(kind, "dropping background request: no worker attached");
        return Err(Box::new(match post {
            BackgroundPost::Post { peer, msg, .. } => BackgroundRequest::Post { peer, msg },
            BackgroundPost::PostControlFlow { peer, frame, .. } => {
                BackgroundRequest::PostControlFlow { peer, frame }
            }
            BackgroundPost::Broadcast { msg, .. } => BackgroundRequest::Broadcast { msg },
            BackgroundPost::BroadcastControlFlow { frame, .. } => {
                BackgroundRequest::BroadcastControlFlow { frame }
            }
        }));
    };

    match tx.try_send(post) {
        Ok(()) => Ok(()),
        Err(mpsc::TrySendError::Full(post)) => {
            trace!(
                kind,
                "background post queue full; falling back to inline send"
            );
            Err(Box::new(match post {
                BackgroundPost::Post { peer, msg, .. } => BackgroundRequest::Post { peer, msg },
                BackgroundPost::PostControlFlow { peer, frame, .. } => {
                    BackgroundRequest::PostControlFlow { peer, frame }
                }
                BackgroundPost::Broadcast { msg, .. } => BackgroundRequest::Broadcast { msg },
                BackgroundPost::BroadcastControlFlow { frame, .. } => {
                    BackgroundRequest::BroadcastControlFlow { frame }
                }
            }))
        }
        Err(mpsc::TrySendError::Disconnected(post)) => {
            iroha_logger::warn!(kind, "background post channel disconnected");
            Err(Box::new(match post {
                BackgroundPost::Post { peer, msg, .. } => BackgroundRequest::Post { peer, msg },
                BackgroundPost::PostControlFlow { peer, frame, .. } => {
                    BackgroundRequest::PostControlFlow { peer, frame }
                }
                BackgroundPost::Broadcast { msg, .. } => BackgroundRequest::Broadcast { msg },
                BackgroundPost::BroadcastControlFlow { frame, .. } => {
                    BackgroundRequest::BroadcastControlFlow { frame }
                }
            }))
        }
    }
}

impl Actor {
    pub(super) fn send_background_immediate(&self, request: BackgroundRequest) {
        match request {
            BackgroundRequest::Post { peer, msg } => {
                #[cfg(feature = "telemetry")]
                self.telemetry.inc_post_to_peer(&peer);
                let post = Post {
                    data: crate::NetworkMessage::SumeragiBlock(Box::new(msg)),
                    peer_id: peer,
                    priority: Priority::High,
                };
                self.network.post(post);
            }
            BackgroundRequest::PostControlFlow { peer, frame } => {
                #[cfg(feature = "telemetry")]
                self.telemetry.inc_post_to_peer(&peer);
                let post = Post {
                    data: crate::NetworkMessage::SumeragiControlFlow(Box::new(frame)),
                    peer_id: peer,
                    priority: Priority::High,
                };
                self.network.post(post);
            }
            BackgroundRequest::Broadcast { msg } => {
                let broadcast = Broadcast {
                    data: crate::NetworkMessage::SumeragiBlock(Box::new(msg)),
                    priority: Priority::High,
                };
                self.network.broadcast(broadcast);
            }
            BackgroundRequest::BroadcastControlFlow { frame } => {
                let broadcast = Broadcast {
                    data: crate::NetworkMessage::SumeragiControlFlow(Box::new(frame)),
                    priority: Priority::High,
                };
                self.network.broadcast(broadcast);
            }
        }
    }

    pub(super) fn rebroadcast_highest_pending_block(&mut self, now: Instant) {
        let Some((hash, block, height, view)) = self
            .pending
            .pending_blocks
            .iter()
            .max_by_key(|(_, pending)| (pending.height, pending.view))
            .map(|(hash, pending)| (*hash, pending.block.clone(), pending.height, pending.view))
        else {
            return;
        };
        let cooldown = self.payload_rebroadcast_cooldown();
        if !pending_replay_due(
            self.pending.pending_replay_last_sent.get(&hash).copied(),
            now,
            cooldown,
        ) {
            return;
        }
        let msg = BlockMessage::BlockCreated(super::message::BlockCreated { block });
        self.schedule_background(BackgroundRequest::Broadcast { msg });
        self.pending.pending_replay_last_sent.insert(hash, now);
        debug!(
            height,
            view,
            block = %hash,
            cooldown_ms = cooldown.as_millis(),
            "rebroadcasting pending block after view change"
        );
    }
}
