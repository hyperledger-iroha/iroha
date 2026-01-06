//! Background send/queue helpers for consensus messages.

use iroha_logger::prelude::*;

use super::*;

#[cfg(feature = "telemetry")]
pub(super) fn dispatch_background_request(
    tx_opt: Option<&mpsc::SyncSender<BackgroundPost>>,
    request: BackgroundRequest,
    telemetry: &Telemetry,
) -> Result<(), Box<BackgroundRequest>> {
    let allow_blocking = background_request_allows_blocking(&request);
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
    let request_from_post = |post| match post {
        BackgroundPost::Post { peer, msg, .. } => BackgroundRequest::Post { peer, msg },
        BackgroundPost::PostControlFlow { peer, frame, .. } => {
            BackgroundRequest::PostControlFlow { peer, frame }
        }
        BackgroundPost::Broadcast { msg, .. } => BackgroundRequest::Broadcast { msg },
        BackgroundPost::BroadcastControlFlow { frame, .. } => {
            BackgroundRequest::BroadcastControlFlow { frame }
        }
    };

    let Some(tx) = tx_opt else {
        trace!(kind, "dropping background request: no worker attached");
        super::status::record_bg_post_drop(kind);
        telemetry.inc_bg_post_drop(kind);
        return Err(Box::new(request_from_post(post)));
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
            trace!(kind, "background post queue full; applying backpressure");
            if !allow_blocking {
                trace!(kind, "dropping non-blocking background request");
                super::status::record_bg_post_drop(kind);
                telemetry.inc_bg_post_drop(kind);
                return Err(Box::new(request_from_post(post)));
            }
            match tx.send(post) {
                Ok(()) => {
                    telemetry.inc_bg_post_enqueued(kind);
                    if let Some(peer) = peer_for_metrics.as_ref() {
                        telemetry.inc_post_to_peer(peer);
                        telemetry.inc_bg_post_queue_depth_for_peer(peer);
                    }
                    Ok(())
                }
                Err(mpsc::SendError(post)) => {
                    iroha_logger::warn!(kind, "background post channel disconnected");
                    super::status::record_bg_post_drop(kind);
                    telemetry.inc_bg_post_drop(kind);
                    Err(Box::new(request_from_post(post)))
                }
            }
        }
        Err(mpsc::TrySendError::Disconnected(post)) => {
            iroha_logger::warn!(kind, "background post channel disconnected");
            super::status::record_bg_post_drop(kind);
            telemetry.inc_bg_post_drop(kind);
            Err(Box::new(request_from_post(post)))
        }
    }
}

#[cfg(not(feature = "telemetry"))]
pub(super) fn dispatch_background_request(
    tx_opt: Option<&mpsc::SyncSender<BackgroundPost>>,
    request: BackgroundRequest,
) -> Result<(), Box<BackgroundRequest>> {
    let allow_blocking = background_request_allows_blocking(&request);
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
    let request_from_post = |post| match post {
        BackgroundPost::Post { peer, msg, .. } => BackgroundRequest::Post { peer, msg },
        BackgroundPost::PostControlFlow { peer, frame, .. } => {
            BackgroundRequest::PostControlFlow { peer, frame }
        }
        BackgroundPost::Broadcast { msg, .. } => BackgroundRequest::Broadcast { msg },
        BackgroundPost::BroadcastControlFlow { frame, .. } => {
            BackgroundRequest::BroadcastControlFlow { frame }
        }
    };

    let Some(tx) = tx_opt else {
        trace!(kind, "dropping background request: no worker attached");
        super::status::record_bg_post_drop(kind);
        return Err(Box::new(request_from_post(post)));
    };

    match tx.try_send(post) {
        Ok(()) => Ok(()),
        Err(mpsc::TrySendError::Full(post)) => {
            trace!(kind, "background post queue full; applying backpressure");
            if !allow_blocking {
                trace!(kind, "dropping non-blocking background request");
                super::status::record_bg_post_drop(kind);
                return Err(Box::new(request_from_post(post)));
            }
            match tx.send(post) {
                Ok(()) => Ok(()),
                Err(mpsc::SendError(post)) => {
                    iroha_logger::warn!(kind, "background post channel disconnected");
                    super::status::record_bg_post_drop(kind);
                    Err(Box::new(request_from_post(post)))
                }
            }
        }
        Err(mpsc::TrySendError::Disconnected(post)) => {
            iroha_logger::warn!(kind, "background post channel disconnected");
            super::status::record_bg_post_drop(kind);
            Err(Box::new(request_from_post(post)))
        }
    }
}

fn background_request_allows_blocking(request: &BackgroundRequest) -> bool {
    // Always allow backpressure for consensus payloads, including RBC chunks.
    match request {
        BackgroundRequest::Post { .. }
        | BackgroundRequest::Broadcast { .. }
        | BackgroundRequest::PostControlFlow { .. }
        | BackgroundRequest::BroadcastControlFlow { .. } => true,
    }
}

impl Actor {
    pub(super) fn rebroadcast_highest_pending_block(&mut self, now: Instant) {
        let Some((hash, block, height, view)) = self
            .pending
            .pending_blocks
            .iter()
            .filter(|(_, pending)| !pending.aborted)
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
