//! Publication presentation controls; no generated state, signed success, worker or HTTP fixture.

use super::{
    tests::{click_label, render_frame, text_position},
    *,
};

#[test]
fn publication_form_uses_typed_selectors_and_original_operation_ids() {
    let mut form = PublicationForm::default();
    assert_eq!(form.manifest, ".");
    assert!(matches!(
        form.action(PublicationAction::Begin).unwrap(),
        GeneratedPublishAction::Begin {
            package: None,
            detach: false
        }
    ));
    form.package = "dev.universal/example".into();
    form.detach = true;
    match form.action(PublicationAction::Begin).unwrap() {
        GeneratedPublishAction::Begin {
            package: Some(package),
            detach,
        } => {
            assert_eq!(package.to_string(), "dev.universal/example");
            assert!(detach);
        }
        _ => panic!("exact typed Begin selection expected"),
    }
    form.package = "not a package selector".into();
    assert!(form.action(PublicationAction::Begin).is_err());
    let original = "31".repeat(32);
    form.operation_id = format!("  {original}  ");
    // Begin-only fields never replace or prevent selection of the original recovery operation.
    for action in [PublicationAction::Resume, PublicationAction::Recover] {
        let selected = form.action(action).unwrap();
        let operation_id = match selected {
            GeneratedPublishAction::Resume { operation_id }
                if action == PublicationAction::Resume =>
            {
                operation_id
            }
            GeneratedPublishAction::Recover { operation_id }
                if action == PublicationAction::Recover =>
            {
                operation_id
            }
            _ => panic!("exact original recovery action expected"),
        };
        assert_eq!(operation_id.to_string(), original);
    }
    for invalid in [
        String::new(),
        "00".repeat(32),
        "AB".repeat(32),
        "31".repeat(31),
    ] {
        form.operation_id = invalid;
        assert!(form.action(PublicationAction::Resume).is_err());
        assert!(form.action(PublicationAction::Recover).is_err());
    }
}

#[test]
fn publication_form_requires_explicit_click_and_respects_disabled_actions() {
    for (label, expected) in [
        ("Publish package", PublicationAction::Begin),
        ("Resume publication", PublicationAction::Resume),
        ("Recover package files", PublicationAction::Recover),
    ] {
        for available in [false, true] {
            let context = egui::Context::default();
            let mut form = PublicationForm {
                operation_id: "31".repeat(32),
                ..Default::default()
            };
            let mut requested = None;
            let mut draw = |context: &egui::Context| {
                egui::CentralPanel::default().show(context, |ui| {
                    if let Some(action) = form.show(ui, available) {
                        requested = Some(action);
                    }
                });
            };
            let _ = render_frame(&context, Vec::new(), &mut draw);
            click_label(&context, label, &mut draw);
            assert_eq!(requested, available.then_some(expected));
        }
    }
    for label in ["Resume publication", "Recover package files"] {
        let context = egui::Context::default();
        let mut form = PublicationForm::default();
        let mut requested = None;
        click_label(&context, label, |context| {
            egui::CentralPanel::default().show(context, |ui| {
                if let Some(action) = form.show(ui, true) {
                    requested = Some(action);
                }
            });
        });
        assert!(
            requested.is_none(),
            "recovery needs an original operation ID"
        );
    }
}

#[test]
fn malformed_publication_action_never_enters_background_work() {
    let mut desktop = Desktop::model(PathBuf::from("unused"));
    desktop.publication.package = "invalid selector".into();
    desktop.publish_package(PublicationAction::Begin);
    assert!(
        desktop
            .error
            .as_deref()
            .unwrap()
            .contains("Invalid package selector")
    );
    assert!(!desktop.busy);
    assert!(matches!(
        desktop.receiver.try_recv(),
        Err(mpsc::TryRecvError::Empty)
    ));
    desktop.publication.operation_id = "00".repeat(32);
    desktop.publish_package(PublicationAction::Resume);
    assert!(
        desktop
            .error
            .as_deref()
            .unwrap()
            .contains("Invalid publication operation ID")
    );
    assert!(!desktop.busy);
    assert!(matches!(
        desktop.receiver.try_recv(),
        Err(mpsc::TryRecvError::Empty)
    ));
    desktop.busy = true;
    desktop.error = Some("original pending work".into());
    desktop.publish_package(PublicationAction::Recover);
    assert_eq!(desktop.error.as_deref(), Some("original pending work"));
    assert!(desktop.busy);
    assert!(matches!(
        desktop.receiver.try_recv(),
        Err(mpsc::TryRecvError::Empty)
    ));
}

#[test]
fn publication_output_retains_status_and_both_streams_after_refresh_failure() {
    let mut desktop = Desktop::model(PathBuf::from("unused"));
    desktop.busy = true;
    desktop.receipt = Some("unrelated contract receipt".into());
    desktop
        .sender
        .send((
            0,
            Message::Published {
                // These are presentation bytes, not a manufactured GeneratedPublishOutcome or receipt.
                result: Ok(PublicationOutput {
                    exit_code: 7,
                    stdout: "retained operation 123\n".into(),
                    stderr: "exact original pending diagnostic\n".into(),
                }),
                refreshed: Err("observation unavailable".into()),
            },
        ))
        .unwrap();
    desktop.poll();
    assert!(!desktop.busy);
    let result = desktop.publication_output.as_ref().unwrap();
    assert_eq!(result.exit_code, 7);
    assert_eq!(result.stdout, "retained operation 123\n");
    assert_eq!(result.stderr, "exact original pending diagnostic\n");
    assert_eq!(
        desktop.receipt.as_deref(),
        Some("unrelated contract receipt")
    );
    assert!(desktop.selected.is_none());
    assert!(desktop.notice.is_none());
    assert!(
        desktop
            .error
            .as_deref()
            .unwrap()
            .contains("observation unavailable")
    );
    let context = egui::Context::default();
    let output = render_frame(&context, Vec::new(), &mut |context| {
        egui::CentralPanel::default().show(context, |ui| result.show(ui));
    });
    assert!(text_position(&output, "Publication output · exit status 7").is_some());
    assert!(text_position(&output, "exact original pending diagnostic\n").is_some());
}

#[test]
fn stale_publication_completion_cannot_replace_current_workspace_or_output() {
    let mut desktop = Desktop::model(PathBuf::from("unused"));
    desktop.epoch = 2;
    desktop.busy = true;
    desktop.publication.operation_id = "31".repeat(32);
    desktop.publication_output = Some(PublicationOutput {
        exit_code: 0,
        stdout: "current presentation".into(),
        stderr: String::new(),
    });
    desktop
        .sender
        .send((
            1,
            Message::Published {
                result: Err("stale workspace failure".into()),
                refreshed: Err("stale observation".into()),
            },
        ))
        .unwrap();
    desktop.poll();
    assert!(desktop.busy);
    assert!(desktop.error.is_none());
    assert_eq!(
        desktop.publication_output.as_ref().unwrap().stdout,
        "current presentation"
    );
    assert_eq!(desktop.publication.operation_id, "31".repeat(32));
    desktop.clear_network();
    assert!(desktop.publication_output.is_none());
    assert!(desktop.publication.operation_id.is_empty());
}

#[test]
fn publication_tab_never_invokes_contract_deployment_or_claims_ready() {
    let mut desktop = Desktop::model(PathBuf::from("unused"));
    desktop.view = View::Packages;
    desktop.contract_path = "original.ko".into();
    desktop.journal_path = "original-deployment".into();
    desktop.receipt = Some("original contract receipt".into());
    let context = egui::Context::default();
    for label in [
        "Publish package",
        "Resume publication",
        "Recover package files",
    ] {
        click_label(&context, label, |context| desktop.show(context));
    }
    assert!(
        !desktop.busy,
        "no workspace: no publication can be dispatched"
    );
    assert!(desktop.selected.is_none());
    assert!(desktop.review.is_none());
    assert!(desktop.publication_output.is_none());
    assert_eq!(desktop.contract_path, "original.ko");
    assert_eq!(desktop.journal_path, "original-deployment");
    assert_eq!(
        desktop.receipt.as_deref(),
        Some("original contract receipt")
    );
    assert!(View::ALL.contains(&View::Packages));
}
