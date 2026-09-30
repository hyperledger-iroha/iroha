//! Copy-mode and crash-prefix controls run with a restrictive child umask.
use super::*;

#[test]
fn dispatcher_transition_private_copy_modes_survive_restrictive_umask() {
    const CHILD: &str = "IROHA_DISPATCHER_TRANSITION_COPY_TEST_CHILD";
    if std::env::var_os(CHILD).is_none() {
        for mask in ["077", "0277"] {
            let result = std::process::Command::new("/bin/sh")
                .args(["-c", "umask \"$1\"; shift; exec \"$@\"", "transition-copy-test", mask])
                .arg(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "taira_public_reset::host::dispatcher_transition::tests::copy::dispatcher_transition_private_copy_modes_survive_restrictive_umask",
                    "--nocapture",
                ])
                .env(CHILD, "1")
                .output()
                .unwrap();
            let stdout = String::from_utf8_lossy(&result.stdout);
            assert!(
                result.status.success() && stdout.contains("test result: ok. 1 passed; 0 failed;"),
                "child copy regression under umask{mask} did not execute and pass exactly once: {stdout}{}",
                String::from_utf8_lossy(&result.stderr)
            );
        }
        return;
    }
    let f = fixture();
    run(&f, Action::Apply).unwrap();
    assert_live(&f, true);
    assert_eq!(
        fs::metadata(&f.plan.predecessor.dispatcher.path)
            .unwrap()
            .mode()
            & 0o7777,
        0o755
    );
    for guard in &f.plan.predecessor.guards {
        assert_eq!(fs::metadata(&guard.path).unwrap().mode() & 0o7777, 0o600);
    }
    run(&f, Action::Rollback).unwrap();
    assert_live(&f, false);
    assert_eq!(
        fs::metadata(&f.plan.predecessor.dispatcher.path)
            .unwrap()
            .mode()
            & 0o7777,
        0o755
    );

    for (case, content, mode, accepted) in [
        ("private-prefix", b"old".as_slice(), 0o600, true),
        (
            "before-mode",
            b"old native dispatcher".as_slice(),
            0o600,
            true,
        ),
        (
            "before-rename",
            b"old native dispatcher".as_slice(),
            0o755,
            true,
        ),
        ("incomplete-executable", b"old".as_slice(), 0o755, false),
        ("foreign-prefix", b"foreign".as_slice(), 0o600, false),
        (
            "wrong-mode",
            b"old native dispatcher".as_slice(),
            0o700,
            false,
        ),
    ] {
        let f = fixture();
        let target = f._temp.path().join(case);
        let partial = f._temp.path().join(format!(".{case}.partial"));
        file(&partial, content, mode);
        let before = fs::metadata(&partial).unwrap();
        let result = storage::copy::copy_exact(
            Path::new(&f.plan.predecessor.dispatcher.path),
            &target,
            &f.plan.predecessor.dispatcher,
        );
        if accepted {
            result.unwrap_or_else(|error| panic!("{case}: {error:#}"));
            assert_eq!(fs::read(&target).unwrap(), b"old native dispatcher");
            let after = fs::metadata(&target).unwrap();
            assert_eq!((after.dev(), after.ino()), (before.dev(), before.ino()));
            assert_eq!(after.mode() & 0o7777, 0o755);
            assert!(!partial.exists());
        } else {
            assert!(result.is_err(), "accepted {case}");
            assert!(!target.exists());
            assert_eq!(fs::read(&partial).unwrap(), content);
            let after = fs::metadata(&partial).unwrap();
            assert_eq!(
                (after.dev(), after.ino(), after.mode()),
                (before.dev(), before.ino(), before.mode())
            );
        }
    }
}
