//! Wake the native window when work completes or a bounded validator stream has data.

use super::Message;
use eframe::egui;
use std::{
    sync::{Arc, mpsc},
    task::{Wake, Waker},
};

struct WindowWake(egui::Context);

impl Wake for WindowWake {
    fn wake(self: Arc<Self>) {
        self.wake_by_ref();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.0.request_repaint_of(egui::ViewportId::ROOT);
    }
}

#[derive(Clone)]
pub(super) struct UiSender {
    sender: mpsc::Sender<(u64, Message)>,
    pub(super) waker: Waker,
}

impl UiSender {
    pub(super) fn new(sender: mpsc::Sender<(u64, Message)>, waker: Waker) -> Self {
        Self { sender, waker }
    }

    pub(super) fn bind_window(&mut self, context: &egui::Context) {
        self.waker = Waker::from(Arc::new(WindowWake(context.clone())));
    }

    pub(super) fn spawn(&self, epoch: u64, work: impl FnOnce() -> Message + Send + 'static) {
        let sender = self.clone();
        std::thread::spawn(move || {
            let _ = sender.send((epoch, work()));
        });
    }

    pub(super) fn send(&self, update: (u64, Message)) -> Result<(), ()> {
        self.sender.send(update).map_err(|_| ())?;
        self.waker.wake_by_ref();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_background_update_wakes_the_window_after_the_message_is_available() {
        let context = egui::Context::default();
        let (tx, rx) = mpsc::channel();
        let mut sender = UiSender::new(tx, Waker::noop().clone());
        sender.bind_window(&context);
        let (observed, wake) = mpsc::channel();
        let rx = std::sync::Mutex::new(rx);
        context.set_request_repaint_callback(move |info| {
            assert_eq!(info.viewport_id, egui::ViewportId::ROOT);
            assert!(info.delay.is_zero());
            let (epoch, Message::Progress(text)) = rx.lock().unwrap().try_recv().unwrap() else {
                panic!("expected progress");
            };
            observed.send((epoch, text)).unwrap();
        });
        sender.spawn(7, || Message::Progress("ready".into()));
        assert_eq!(
            wake.recv_timeout(std::time::Duration::from_secs(5))
                .unwrap(),
            (7, "ready".into())
        );
    }
}
