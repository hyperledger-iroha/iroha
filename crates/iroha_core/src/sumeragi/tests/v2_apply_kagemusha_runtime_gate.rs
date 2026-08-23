#[test]
fn production_proposal_validation_enforces_kagemusha_runtime_projection() {
    let fixture = ApplyFixture::new_for_kagemusha_runtime_projection();

    fixture
        .service
        .validate_candidate(&fixture.context, &fixture.body)
        .expect("the exact startup projection must validate the proposal");

    for (local, label) in [(None, "missing"), (Some([0x56; 32]), "mismatched")] {
        let (service, state) = fixture.restart_service_with_kagemusha_runtime_projection(local);
        let error = match service.validate_candidate(&fixture.context, &fixture.body) {
            Ok(_) => panic!("{label} projection must reject proposal validation"),
            Err(error) => error,
        };
        assert!(
            matches!(&error, V2ApplyError::Validation(reason) if reason.contains("active Kagemusha V4 release requires a different complete runtime projection")),
            "unexpected {label} proposal rejection: {error}"
        );
        assert_eq!(state.committed_height(), 0);
    }
}

#[test]
fn production_commit_apply_enforces_kagemusha_runtime_projection() {
    for (local, label) in [(None, "missing"), (Some([0x56; 32]), "mismatched")] {
        let fixture = ApplyFixture::new_for_kagemusha_runtime_projection();
        let (service, state) = fixture.restart_service_with_kagemusha_runtime_projection(local);
        let mut store = fixture.reopen_body_store();
        let error = match service.execute(&fixture.context, &mut store, &fixture.task) {
            Ok(_) => panic!("{label} projection must reject Commit apply"),
            Err(error) => error,
        };
        assert!(
            matches!(&error, V2ApplyError::Validation(reason) if reason.contains("active Kagemusha V4 release requires a different complete runtime projection")),
            "unexpected {label} Commit rejection: {error}"
        );
        assert_eq!(state.committed_height(), 0);
        assert_eq!(fixture.kura.exact_durable_blocks_count().unwrap(), 0);
    }

    let fixture = ApplyFixture::new_for_kagemusha_runtime_projection();
    let mut store = fixture.reopen_body_store();
    fixture
        .execute(&mut store)
        .expect("the exact startup projection must permit Commit apply");
    fixture.assert_complete();
}
