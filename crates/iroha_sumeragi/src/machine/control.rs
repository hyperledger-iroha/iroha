//! Applied-parent control production and source-bound peer partial delivery.

use super::Core;
use crate::{
    api::{Action, ApplicationControlContext},
    message::{ApplicationControl, WireMessage},
    types::PublicKey,
};

impl Core {
    /// The application may only work from the exact current, fully applied parent.
    pub(super) fn application_control_context(&self) -> Option<ApplicationControlContext> {
        (!self.awaiting
            && self.tip.height == self.applied
            && self.height == self.tip.height.checked_add(1)?)
        .then_some(ApplicationControlContext {
            instance: self.instance,
            epoch: self.cfg.epoch.id,
            height: self.height,
            parent_hash: self.tip.block_hash,
            parent_result: self.tip.result,
        })
    }

    /// Every current signing member drives the one process-lived producer, independent of view
    /// leadership. Retransmission is bounded by the existing rebroadcast cadence.
    pub(super) fn drive_application_control(&mut self) {
        let Some(context) = self.application_control_context() else {
            return;
        };
        if self.signer().is_none() {
            return;
        }
        if self.control_drive.is_some_and(|(prior, at)| {
            prior == context && self.now < at.saturating_add(self.local.rebroadcast_interval)
        }) {
            return;
        }
        if self.control_drive.is_none_or(|(prior, _)| prior != context) {
            self.control_received.clear();
        }
        self.control_drive = Some((context, self.now));
        self.out.push(Action::DriveApplicationControl { context });
    }

    /// A peer identity is necessary but insufficient: the application verifies the partial's
    /// session, index and signature independently, against its own applied State.
    pub(super) fn on_application_control(&mut self, from: &PublicKey, message: ApplicationControl) {
        if self.application_control_context() != Some(message.context)
            || !self.cfg.committee.contains(from)
            || message.bytes.is_empty()
            || self.signer().is_none()
        {
            return;
        }
        if self
            .control_received
            .get(from)
            .is_some_and(|at| self.now < at.saturating_add(self.local.rebroadcast_interval))
        {
            return;
        }
        self.control_received.insert(from.clone(), self.now);
        self.out.push(Action::ReceiveApplicationControl {
            from: from.clone(),
            message,
        });
    }

    /// A completion from a prior source cannot broadcast into a new height or authority.
    pub(super) fn on_application_control_built(&mut self, message: ApplicationControl) {
        if self.application_control_context() != Some(message.context)
            || message.bytes.is_empty()
            || self.signer().is_none()
        {
            return;
        }
        self.broadcast(
            self.members_except_me(),
            WireMessage::ApplicationControl(message),
        );
    }
}
