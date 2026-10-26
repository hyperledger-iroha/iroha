//! Scope-role parity and exact retained-preimage refusal tests; no authority is created.
use super::*;

#[test]
fn scope_preimages_match_native_roles_and_known_answers() {
    let audience = b"community-operator".to_vec();
    let release = b"\0selected release\xff".to_vec();
    let origin = b"https://wallet.example:8443".to_vec();
    let selected = from_originals(audience.clone(), release.clone(), origin.clone()).unwrap();
    assert_eq!(selected.audience, audience);
    assert_eq!(selected.release, release);
    assert_eq!(selected.service_origin, origin);
    use KagemushaEnrollmentPermitScopeRoleV1 as Role;
    // These are the exact role/preimage pairs used by EnrollmentConfigV1::scopes.
    for (actual, role, original, expected) in [
        (
            selected.operator_digest,
            Role::Fi,
            audience.as_slice(),
            "1606e856e034b1e2b2634ff0d872ed78627e8f7f96ef322c5aaa1a6032d902a4",
        ),
        (
            selected.release_digest,
            Role::Release,
            release.as_slice(),
            "98812b936fc0499d20a904c119bf32abeb761cab864fbffe56acacfb26b60ce5",
        ),
        (
            selected.service_origin_digest,
            Role::ServiceOrigin,
            origin.as_slice(),
            "e13fb3a253c9e475c90af9f1f50484ba8de1ba8ef523de79ba746e15c76df95c",
        ),
    ] {
        assert_eq!(
            actual,
            kagemusha_enrollment_permit_scope_digest_v1(role, original).unwrap()
        );
        assert_eq!(hex::encode(actual), expected);
        assert_ne!(
            actual,
            kagemusha_enrollment_permit_scope_digest_v1(Role::Actor, original).unwrap()
        );
    }
    assert_ne!(
        selected.operator_digest,
        kagemusha_enrollment_permit_scope_digest_v1(Role::Fi, b"another-operator").unwrap()
    );
}

#[test]
fn scope_originals_require_matching_pins_and_retained_custody() {
    let base = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../target/qualification/issuer-provisioning-tests");
    fs::create_dir_all(&base).unwrap();
    let temporary = tempfile::tempdir_in(base.canonicalize().unwrap()).unwrap();
    let root = temporary.path().canonicalize().unwrap();
    let original = |name: &str, bytes: &[u8]| {
        let path = root.join(name);
        fs::write(&path, bytes).unwrap();
        Original {
            path,
            sha256: hash(bytes),
        }
    };
    let mut selected = Originals {
        audience: original("audience", b"community-operator"),
        release: original("release", b"exact release DATA"),
        service_origin: original("origin", b"https://wallet.example"),
    };
    let mut inputs = Inputs(Vec::new());
    let scopes = read(&selected, &mut inputs).unwrap();
    assert_eq!(scopes.release, b"exact release DATA");
    inputs.revalidate().unwrap();
    for role in 0..3 {
        let pin = match role {
            0 => &mut selected.audience.sha256,
            1 => &mut selected.release.sha256,
            _ => &mut selected.service_origin.sha256,
        };
        let saved = std::mem::replace(pin, hex::encode([99; 32]));
        assert!(read(&selected, &mut Inputs(Vec::new())).is_err());
        match role {
            0 => selected.audience.sha256 = saved,
            1 => selected.release.sha256 = saved,
            _ => selected.service_origin.sha256 = saved,
        }
    }
    fs::write(&selected.release.path, b"replacement release DATA").unwrap();
    assert!(inputs.revalidate().is_err());
    assert!(read(&selected, &mut Inputs(Vec::new())).is_err());
}

#[test]
fn scope_text_and_origin_reject_noncanonical_preimages_without_rewriting() {
    for audience in [
        Vec::new(),
        b" operator".to_vec(),
        b"operator ".to_vec(),
        b"operator\n".to_vec(),
        b"operator\0".to_vec(),
        vec![0xff],
        vec![b'a'; 513],
    ] {
        assert!(
            from_originals(
                audience,
                b"release".to_vec(),
                b"https://wallet.example".to_vec()
            )
            .is_err()
        );
    }
    for origin in [
        "",
        "http://wallet.example",
        "https://wallet.example/",
        "https://wallet.example/path",
        "https://wallet.example?x=1",
        "https://wallet.example#fragment",
        "https://user:password@wallet.example",
        "https://wallet.example:0",
        "https://wallet.example:65536",
        "https://wallet.example:443",
        "HTTPS://wallet.example",
        "https://WALLET.example",
        " https://wallet.example",
        "https://wallet.example\n",
        "https://bad_host.example",
        "https://wallet.example.",
        "https://-wallet.example",
        "https://wallet..example",
    ] {
        assert!(
            from_originals(
                b"operator".to_vec(),
                b"release".to_vec(),
                origin.as_bytes().to_vec()
            )
            .is_err(),
            "{origin:?}"
        );
    }
    assert!(from_originals(b"operator".to_vec(), b"release".to_vec(), vec![0xff]).is_err());
    for origin in [
        "https://wallet.example",
        "https://127.0.0.1:8443",
        "https://[::1]:8443",
    ] {
        let selected = from_originals(
            b"operator".to_vec(),
            b"release".to_vec(),
            origin.as_bytes().to_vec(),
        )
        .unwrap();
        assert_eq!(selected.service_origin, origin.as_bytes());
    }
}

#[test]
fn scope_release_is_exact_opaque_bounded_data() {
    for length in [0, PUBLIC_MAX + 1] {
        assert!(
            from_originals(
                b"operator".to_vec(),
                vec![7; length],
                b"https://wallet.example".to_vec()
            )
            .is_err()
        );
    }
    let release = vec![0xff; PUBLIC_MAX];
    let scopes = from_originals(
        vec![b'a'; 512],
        release.clone(),
        b"https://wallet.example".to_vec(),
    )
    .unwrap();
    assert_eq!(scopes.release, release);
}
