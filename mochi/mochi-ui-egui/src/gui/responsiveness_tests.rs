//! Regressions for idle frames, wakeups and background reads.

use super::{tests::run_headless_frame, *};

#[test]
fn a_burst_of_progress_messages_leaves_room_for_the_next_frame() {
    let mut desktop = Desktop::model(PathBuf::from("unused"));
    for index in 0..=UPDATES_PER_FRAME {
        desktop
            .sender
            .send((desktop.epoch, Message::Progress(index.to_string())))
            .unwrap();
    }
    desktop.poll();
    assert_eq!(desktop.activity.len(), UPDATES_PER_FRAME);
    desktop.poll();
    assert_eq!(
        desktop.activity.back().unwrap().text,
        UPDATES_PER_FRAME.to_string()
    );
}

#[test]
fn pending_balance_refresh_leaves_controls_enabled_and_cannot_overwrite_a_new_action() {
    let mut desktop = Desktop::model(PathBuf::from("unused"));
    let (finish, release) = mpsc::channel();
    desktop.spawn_observation(ObservationKind::Ledger, move || {
        release.recv().unwrap();
        Message::Ledger(Err("stale balance read".into()))
    });
    assert!(!desktop.busy);
    desktop.spawn_observation(ObservationKind::Ledger, || panic!("duplicate balance read"));
    desktop.spawn(|| Message::Logs(Ok("current action".into())));
    assert!(desktop.busy && desktop.foreground);
    assert!(desktop.observation.is_none());
    assert!(desktop.ledger_stale);
    finish.send(()).unwrap();
    let update = desktop
        .receiver
        .recv_timeout(Duration::from_secs(5))
        .unwrap();
    desktop.sender.send(update).unwrap();
    desktop.poll();
    assert_eq!(desktop.logs, "current action");
    assert!(desktop.ledger_error.is_none());
    assert!(!desktop.busy);
}

#[test]
fn late_stream_setup_cannot_reconnect_a_disconnected_peer() {
    let mut desktop = Desktop::model(PathBuf::from("unused"));
    desktop.opening_streams = true;
    let request = desktop.stream_request;
    desktop.disconnect_streams();
    desktop.busy = true;
    desktop
        .sender
        .send((
            desktop.epoch,
            Message::StreamsOpened {
                request,
                streams: Err("previous peer".into()),
            },
        ))
        .unwrap();
    desktop.poll();
    assert!(desktop.error.is_none());
    assert!(!desktop.opening_streams && desktop.busy);
    assert!(desktop.blocks.is_none() && desktop.events.is_none());
}

#[test]
fn stream_arrival_wakes_an_idle_window_and_backlog_continues_next_frame() {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    struct Counter(AtomicUsize);
    impl std::task::Wake for Counter {
        fn wake(self: Arc<Self>) {
            self.0.fetch_add(1, Ordering::Relaxed);
        }
    }
    let counter = Arc::new(Counter(AtomicUsize::new(0)));
    let waker = Waker::from(counter.clone());
    let (tx, rx) = tokio::sync::broadcast::channel(128);
    let mut stream = BroadcastStream::new(rx);
    let mut activity = VecDeque::new();
    let mut seen = Vec::new();
    let mut drain = |seen: &mut Vec<usize>| {
        drain_stream(&mut stream, &waker, &mut activity, usize::to_string, |n| {
            seen.push(*n)
        });
    };
    drain(&mut seen);
    assert_eq!(counter.0.load(Ordering::Relaxed), 0);
    for n in 0..100 {
        tx.send(n).unwrap();
    }
    let arrivals = counter.0.load(Ordering::Relaxed);
    assert!(arrivals > 0);
    drain(&mut seen);
    assert_eq!(seen.len(), 64);
    assert!(counter.0.load(Ordering::Relaxed) > arrivals);
    drain(&mut seen);
    assert_eq!(seen, (0..100).collect::<Vec<_>>());
}

#[test]
fn idle_home_sleeps_until_input_or_new_data() {
    let mut desktop = Desktop::model(PathBuf::from("unused"));
    let context = egui::Context::default();
    let mut delay = Duration::ZERO;
    // Settle startup layout, font uploads and egui's initial immediate repaints.
    for frame in 0..8 {
        let input = egui::RawInput {
            time: Some(f64::from(frame)),
            ..Default::default()
        };
        let output = run_headless_frame(&context, input, |ui| desktop.show(ui));
        delay = output.viewport_output[&egui::ViewportId::ROOT].repaint_delay;
    }
    assert_eq!(delay, Duration::MAX, "idle Home still schedules a repaint");
}

#[test]
fn balance_queries_are_deferred_until_a_view_uses_them() {
    let mut desktop = Desktop::model(PathBuf::from("unused"));
    for view in View::ALL {
        desktop.view = view;
        assert_eq!(
            desktop.wants_ledger(),
            matches!(view, View::Home | View::Accounts | View::Composer)
        );
    }
    desktop.view = View::Accounts;
    desktop.ledger_error = Some("last read failed".into());
    assert!(!desktop.wants_ledger());
    desktop.ledger_stale = true;
    assert!(desktop.wants_ledger());
}
