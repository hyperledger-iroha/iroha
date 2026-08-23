state_test! { sync kagemusha_runtime_projection_identity_is_install_once
    let state = blank_state();
    assert!(state.install_kagemusha_runtime_effective_config_sha256([0; 32]).is_err());
    state
        .install_kagemusha_runtime_effective_config_sha256([0x51; 32])
        .expect("install nonzero projection identity");
    state
        .install_kagemusha_runtime_effective_config_sha256([0x51; 32])
        .expect("idempotent reinstall is accepted");
    assert!(
        state
            .install_kagemusha_runtime_effective_config_sha256([0x52; 32])
            .is_err()
    );
    state
        .require_committed_kagemusha_runtime_effective_config()
        .expect("an empty lifecycle has no runtime lock");
}

#[test]
fn concurrent_runtime_projection_install_accepts_only_one_distinct_digest() {
    const INSTALLERS: usize = 16;
    let state = Arc::new(blank_state());
    let barrier = Arc::new(Barrier::new(INSTALLERS));
    let installers = (0..INSTALLERS)
        .map(|index| {
            let state = Arc::clone(&state);
            let barrier = Arc::clone(&barrier);
            std::thread::spawn(move || {
                let byte = u8::try_from(index + 1).expect("installer index fits u8");
                let digest = [byte; 32];
                barrier.wait();
                (
                    digest,
                    state.install_kagemusha_runtime_effective_config_sha256(digest),
                )
            })
        })
        .collect::<Vec<_>>();
    let results = installers
        .into_iter()
        .map(|installer| {
            installer
                .join()
                .expect("runtime digest installer must not panic")
        })
        .collect::<Vec<_>>();
    let winners = results
        .iter()
        .filter(|(_, result)| result.is_ok())
        .map(|(digest, _)| *digest)
        .collect::<Vec<_>>();
    assert_eq!(winners.len(), 1, "only the installed digest may succeed");
    state
        .install_kagemusha_runtime_effective_config_sha256(winners[0])
        .expect("the winning digest remains idempotent");
    for (digest, result) in results {
        if digest != winners[0] {
            assert!(result.is_err(), "a distinct concurrent digest was accepted");
            assert!(
                state
                    .install_kagemusha_runtime_effective_config_sha256(digest)
                    .is_err(),
                "a losing digest must remain rejected"
            );
        }
    }
}
