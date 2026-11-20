use super::*;
use crate::{certificate, credential, ipa::Gens};
use ff::{Field, PrimeField};

fn fixture() -> crate::chain_fixture::Chain {
    crate::chain_fixture::new(&crate::ipa::Gens::<iroha_pasta::Eq>::new(4096))
}

#[test]
fn program_bundle_frame_uses_its_explicit_bound() {
    let default_archive_limit = norito::core::max_archive_len();
    let mut bundle = ProgramBundle {
        version: 1,
        primary: std::array::from_fn(|_| vec![1]),
        secondary: std::array::from_fn(|_| vec![2]),
    };
    let overhead = bundle.encode().unwrap().len() - 1;
    bundle.primary[0] = vec![0x5a; MAX_PROGRAM_BUNDLE_BYTES - overhead];
    let mut bytes = bundle.encode().unwrap();
    assert_eq!(bytes.len(), MAX_PROGRAM_BUNDLE_BYTES);
    let decoded = ProgramBundle::decode(&bytes).unwrap();
    assert_eq!(decoded.primary, bundle.primary);
    assert_eq!(decoded.secondary, bundle.secondary);
    drop(decoded);
    bundle.primary[0].push(0);
    assert!(matches!(bundle.encode(), Err(Error::ResourceLimit)));
    bytes.push(0);
    assert!(matches!(
        ProgramBundle::decode(&bytes),
        Err(Error::ResourceLimit)
    ));
    bytes.pop();
    assert_eq!(norito::core::max_archive_len(), default_archive_limit);
    // A valid full bundle must also satisfy any stricter surrounding budget.
    let strict = norito::DecodeLimits::new(1024, 1024, 4096, 4096, 16);
    assert!(
        norito::core::with_decode_limits_scope(strict, || ProgramBundle::decode(&bytes)).is_err()
    );
    bytes.push(0);
    assert!(ProgramBundle::decode(&bytes).is_err());
    bytes.pop();
    *bytes.last_mut().unwrap() ^= 1;
    assert!(ProgramBundle::decode(&bytes).is_err());
}

fn small_generators() -> (Gens<iroha_pasta::Eq>, Gens<iroha_pasta::Ep>) {
    (Gens::new(1 << 12), Gens::new(1 << 10))
}

fn test_manifest(scheme: [u8; 32], signer: &certificate::Certificate) -> ParameterManifest {
    let (gp, gs) = small_generators();
    engine::manifest_for(
        scheme,
        [8; 32],
        std::array::from_fn(|k| [[1 + k as u8; 32], [9 + k as u8; 32]]),
        ((28, 26), (27, 25)),
        (&gp, &gs),
        ("test-network", "test-chain"),
        signer,
    )
    .unwrap()
}

fn root_secret() -> BigUint {
    BigUint::from_bytes_be(&engine::TEST_ROOT_SECRET)
}

fn root() -> p256::Point {
    p256::mul(&p256::generator(), &root_secret()).unwrap()
}

/// F3: manifests are signed by a root-certified Artifact-role key, never by
/// the root itself or by a key certified for another role or scheme.
#[test]
fn parameter_manifest_requires_artifact_role_signer() {
    let scheme = [7; 32];
    let artifact = engine::test_artifact_certificate(&scheme);
    let bytes = encode(&test_manifest(scheme, &artifact)).unwrap();
    let signature = engine::test_sign_manifest(&bytes);
    let (original, digest) = engine::authenticate_manifest(&bytes, &signature, &root()).unwrap();
    // Every pin, the profile included, is under the signature.
    for case in 0..4 {
        let mut changed: ParameterManifest = decode(&bytes).unwrap();
        match case {
            0 => changed.primary[0][0] ^= 1,
            1 => changed.dimensions_p[0] -= 1,
            2 => changed.generator_digests[1][0] ^= 1,
            _ => changed.table_vector[0][0] ^= 1,
        }
        let changed_bytes = encode(&changed).unwrap();
        assert!(
            engine::authenticate_manifest(&changed_bytes, &signature, &root()).is_err(),
            "changed pin {case}"
        );
        let replacement_signature = engine::test_sign_manifest(&changed_bytes);
        let (replacement, replacement_digest) =
            engine::authenticate_manifest(&changed_bytes, &replacement_signature, &root()).unwrap();
        assert_eq!(original.relation, replacement.relation);
        assert_ne!(digest, replacement_digest, "changed pin {case}");
    }
    let wrong = p256::mul(&p256::generator(), &BigUint::from(12346u64)).unwrap();
    assert!(engine::authenticate_manifest(&bytes, &signature, &wrong).is_err());
    let mut extra = bytes.clone();
    extra.push(0);
    assert!(engine::authenticate_manifest(&extra, &signature, &root()).is_err());
    // The root signs certificates only.
    let by_root = bootstrap::sign(
        crate::payment::byte_digest(u64::from_le_bytes(*b"kgwparm1"), &bytes),
        &root_secret(),
        "root-signed manifest",
    );
    assert!(matches!(
        engine::authenticate_manifest(&bytes, &by_root, &root()),
        Err(Error::Rejected(m)) if m == "P-256 signature"
    ));
    // The artifact key under a certificate of another role, scheme or root.
    let key = p256::mul(
        &p256::generator(),
        &BigUint::from_bytes_be(&engine::TEST_ARTIFACT_SECRET),
    )
    .unwrap();
    for (role, cert_scheme, issuer, expected) in [
        (1, scheme, root_secret(), "signer certificate role"),
        (3, scheme, root_secret(), "signer certificate role"),
        (2, scheme, root_secret(), "signer certificate role"),
        (5, [9; 32], root_secret(), "signer certificate scheme"),
        (
            5,
            scheme,
            BigUint::from(777u64),
            "signer certificate root signature",
        ),
    ] {
        let cert = certificate::issue_fixture(&cert_scheme, role, &key, 6, &issuer);
        let bytes = encode(&test_manifest(scheme, &cert)).unwrap();
        let signature = engine::test_sign_manifest(&bytes);
        assert!(
            matches!(
                engine::authenticate_manifest(&bytes, &signature, &root()),
                Err(Error::AuthorityMismatch(m)) if m == expected
            ),
            "{expected}"
        );
    }
}

/// G6: the generator namespace, hash-to-curve suite, capacities, derived sets
/// and table construction (with its known answer) are pinned by the signed
/// manifest and recomputed by installation, before any program is used.
#[test]
fn generator_and_table_profile_is_pinned_by_authenticated_parameters() {
    let scheme = [7; 32];
    let manifest = test_manifest(scheme, &engine::test_artifact_certificate(&scheme));
    let bytes = encode(&manifest).unwrap();
    let (gp, gs) = engine::authenticate_profile(&manifest, [12, 10]).unwrap();
    assert_eq!((gp.g.len(), gs.g.len()), (1 << 12, 1 << 10));
    assert!(matches!(
        engine::authenticate_profile(&manifest, [13, 10]),
        Err(Error::AuthorityMismatch("generator capacity"))
    ));
    for (case, expected) in [
        (0, "generator namespace"),
        (1, "hash-to-curve suite"),
        (2, "payment table construction"),
        (3, "generator capacity"),
        (4, "derived generator sets"),
        (5, "derived generator sets"),
        (6, "payment table known answer"),
    ] {
        let mut changed: ParameterManifest = decode(&bytes).unwrap();
        match case {
            0 => changed.generator_domain.push('x'),
            1 => changed.hash_to_curve = changed.hash_to_curve.replace("BLAKE2b", "SHA-256"),
            2 => {
                // Row-major instead of column-major cells.
                changed.table_construction = changed
                    .table_construction
                    .replace("g[512 c + r]", "g[7 r + c]")
            }
            3 => changed.generator_capacity_bits[1] = 11,
            4 => changed.generator_digests[0][31] ^= 1,
            5 => changed.generator_digests[1][0] ^= 1,
            _ => changed.table_vector[1][0] ^= 1,
        }
        // The programs' column capacities stay those of the genuine profile.
        assert!(
            matches!(
                engine::authenticate_profile(&changed, [12, 10]),
                Err(Error::AuthorityMismatch(m)) if m == expected
            ),
            "{expected}"
        );
    }
    // The table needs its 3,584 bases on the P side.
    assert!(engine::generator_profile(&Gens::new(1 << 11), &gs).is_err());
}

fn table_bytes(point: &crate::ipa::Aff<iroha_pasta::Eq>) -> Vec<u8> {
    let (x, y): (iroha_pasta::Fq, iroha_pasta::Fq) =
        Option::from(iroha_pasta::PastaAffine::coordinates(point)).unwrap();
    [x.to_repr(), y.to_repr()].concat()
}

/// G6: a credential correctly signed by the real issuer, but carrying a table
/// commitment that does not recompute from its payment key under the pinned
/// profile, is refused by native credential intake (the same check every
/// head, Payment, preparation and enrollment boundary runs).
#[test]
fn correctly_signed_credential_with_wrong_table_commitment_is_refused() {
    let gens = Gens::<iroha_pasta::Eq>::new(1 << 12);
    let chain = crate::chain_fixture::new(&gens);
    let scheme = chain.sender_context.root.scheme;
    let root = chain.sender_context.root.root.clone();
    let cert = &chain.sender_bootstrap.certificate;
    let good = &chain.sender_bootstrap.credential;
    let intake = |gens: &Gens<iroha_pasta::Eq>, c: &Credential| {
        engine::authenticate_credential_under(&scheme, &root, gens, c, cert)
    };
    assert!(intake(&gens, good).is_ok());
    // g is prefix-stable: another capacity derives the same table bases.
    assert!(intake(&Gens::new(1 << 13), good).is_ok());
    let issued = |table: &[u8]| {
        let mut c = good.clone();
        c.body[195..259].copy_from_slice(table);
        c.signature = bootstrap::sign(
            credential::message(&c),
            &BigUint::from_bytes_be(&[0x26; 32]),
            "correctly issued wrong table",
        );
        c
    };
    let cells = crate::nonnative::wallet_table_values(&chain.sender_key.q);
    let rows = cells[0].len();
    let column_major: Vec<Fp> = cells.iter().flatten().copied().collect();
    let row_major: Vec<Fp> = (0..rows)
        .flat_map(|r| cells.iter().map(move |column| column[r]))
        .collect();
    let shifted: Vec<Fp> = std::iter::once(Fp::ZERO)
        .chain(column_major.iter().copied())
        .collect();
    assert_eq!(
        table_bytes(&crate::spartan::commit_public(&gens, &column_major)),
        good.body[195..259],
        "the fixture issuer used the pinned construction"
    );
    for (label, table) in [
        (
            "another wallet's table",
            chain.receiver_bootstrap.credential.body[195..259].to_vec(),
        ),
        (
            "row-major column order",
            table_bytes(&crate::spartan::commit_public(&gens, &row_major)),
        ),
        (
            "generators shifted by one",
            table_bytes(&crate::spartan::commit_public(&gens, &shifted)),
        ),
    ] {
        let wrong = issued(&table);
        assert!(
            crate::certificate::verify_native(
                &point(&cert.body[35..100]).unwrap(),
                credential::message(&wrong),
                &wrong.signature
            )
            .is_ok(),
            "{label}: the issuer signature itself verifies"
        );
        assert!(
            matches!(
                intake(&gens, &wrong),
                Err(Error::AuthorityMismatch("certified wallet table"))
            ),
            "{label}"
        );
    }
    assert!(matches!(
        intake(&Gens::new(1 << 11), good),
        Err(Error::Malformed("payment table generators"))
    ));
}

/// F3: every signer role is authenticated natively by exact role, scheme,
/// version, key and root signature; tag 2 and unknown tags grant no role.
#[test]
fn signer_certificates_authenticate_exactly_one_role() {
    use certificate::{Role, authenticate_signer, issue_fixture};
    let scheme = [7; 32];
    let key = p256::mul(&p256::generator(), &BigUint::from(77u64)).unwrap();
    let roles = [
        Role::Enrollment,
        Role::RegulatoryPolicy,
        Role::TimeAnchor,
        Role::Artifact,
    ];
    for role in roles {
        let cert = issue_fixture(&scheme, role as u8, &key, 1, &root_secret());
        assert!(matches!(authenticate_signer(&cert, &scheme, &root(), role), Ok(q) if q == key));
        for other in roles.into_iter().filter(|r| *r != role) {
            assert_eq!(
                authenticate_signer(&cert, &scheme, &root(), other),
                Err("signer certificate role")
            );
        }
        assert_eq!(
            authenticate_signer(&cert, &[9; 32], &root(), role),
            Err("signer certificate scheme")
        );
        assert_eq!(
            authenticate_signer(&cert, &[0; 32], &root(), role),
            Err("zero trusted scheme")
        );
        let foreign = issue_fixture(&scheme, role as u8, &key, 1, &BigUint::from(778u64));
        assert_eq!(
            authenticate_signer(&foreign, &scheme, &root(), role),
            Err("signer certificate root signature")
        );
        let mut high = cert.clone();
        let s = BigUint::from_bytes_be(&high.signature[32..]);
        high.signature[32..].copy_from_slice(&certificate::scalar_bytes(&(p256::n() - s)));
        assert_eq!(
            authenticate_signer(&high, &scheme, &root(), role),
            Err("signer certificate root signature")
        );
    }
    for tag in [0u8, 2, 6, 255] {
        let cert = issue_fixture(&scheme, tag, &key, 1, &root_secret());
        for role in roles {
            assert_eq!(
                authenticate_signer(&cert, &scheme, &root(), role),
                Err("signer certificate role"),
                "tag {tag}"
            );
        }
    }
    let resign = |mut cert: certificate::Certificate| {
        cert.signature = bootstrap::sign(certificate::message(&cert), &root_secret(), "resigned");
        cert
    };
    let base = issue_fixture(&scheme, 3, &key, 1, &root_secret());
    let mut version = base.clone();
    version.body[0] = 2;
    assert_eq!(
        authenticate_signer(&resign(version), &scheme, &root(), Role::RegulatoryPolicy),
        Err("signer certificate version")
    );
    let mut compressed = base.clone();
    compressed.body[35] = 2;
    assert_eq!(
        authenticate_signer(
            &resign(compressed),
            &scheme,
            &root(),
            Role::RegulatoryPolicy
        ),
        Err("signer certificate key encoding")
    );
    let mut off_curve = base;
    off_curve.body[99] ^= 1;
    assert_eq!(
        authenticate_signer(&resign(off_curve), &scheme, &root(), Role::RegulatoryPolicy),
        Err("signer certificate key")
    );
}

/// F3: the fee schedule's signer is the RegulatoryPolicy-role key the body
/// names, issued by the installed root for this scheme; every term and the
/// signature are checked natively before a schedule may be held.
#[test]
fn fee_schedule_requires_its_regulatory_policy_signer() {
    use crate::fee_schedule::{SignedFeeSchedule, authenticate, digest, message};
    let scheme = [7; 32];
    let asset = [8; 32];
    let signer_secret = BigUint::from(4242u64);
    let signer_key = p256::mul(&p256::generator(), &signer_secret).unwrap();
    let signer = certificate::issue_fixture(&scheme, 3, &signer_key, 3, &root_secret());
    let schedule = |cert: &certificate::Certificate, edit: &dyn Fn(&mut [u8; 191])| {
        let mut body = [0u8; 191];
        body[..2].copy_from_slice(&1u16.to_le_bytes());
        body[2..34].copy_from_slice(&scheme);
        body[34..66].copy_from_slice(&asset);
        body[66..74].copy_from_slice(&1u64.to_le_bytes());
        body[74..106].fill(0x42);
        body[106..110].copy_from_slice(&25u32.to_le_bytes());
        body[110..126].copy_from_slice(&1u128.to_le_bytes());
        body[126..142].copy_from_slice(&1u128.to_le_bytes());
        body[142..158].copy_from_slice(&1000u128.to_le_bytes());
        body[158] = 1;
        body[159..191].copy_from_slice(&certificate::certificate_digest(cert).to_repr());
        edit(&mut body);
        let mut s = SignedFeeSchedule {
            body,
            signature: [0; 64],
        };
        s.signature = bootstrap::sign(message(&s), &signer_secret, "fee schedule test");
        s
    };
    let check = |s: &SignedFeeSchedule, cert: &certificate::Certificate| {
        authenticate(s, cert, &scheme, &asset, &root())
    };
    let good = schedule(&signer, &|_| {});
    assert_eq!(check(&good, &signer), Ok(digest(&good)));
    // Signer: role, the certificate the body names, issuing root.
    let enrollment = certificate::issue_fixture(&scheme, 1, &signer_key, 3, &root_secret());
    assert_eq!(
        check(&schedule(&enrollment, &|_| {}), &enrollment),
        Err("signer certificate role")
    );
    let other = certificate::issue_fixture(&scheme, 3, &signer_key, 4, &root_secret());
    assert_eq!(check(&good, &other), Err("fee schedule signer certificate"));
    let foreign = certificate::issue_fixture(&scheme, 3, &signer_key, 3, &BigUint::from(9u64));
    assert_eq!(
        check(&schedule(&foreign, &|_| {}), &foreign),
        Err("signer certificate root signature")
    );
    // Body rules, each re-signed by the genuine signer.
    let edits: [(&dyn Fn(&mut [u8; 191]), &str); 8] = [
        (&|b| b[0] = 2, "fee schedule version"),
        (&|b| b[2] ^= 1, "fee schedule scheme"),
        (&|b| b[34] ^= 1, "fee schedule asset"),
        (&|b| b[74..106].fill(0), "fee schedule beneficiary"),
        (
            &|b| b[106..110].copy_from_slice(&10_001u32.to_le_bytes()),
            "fee schedule terms",
        ),
        (
            &|b| b[142..158].copy_from_slice(&0u128.to_le_bytes()),
            "fee schedule terms",
        ),
        (&|b| b[158] = 0, "fee schedule terms"),
        (&|b| b[158] = 3, "fee schedule terms"),
    ];
    for (edit, expected) in edits {
        assert_eq!(check(&schedule(&signer, edit), &signer), Err(expected));
    }
    // Signature: another key under the right certificate, and high S.
    let mut forged = good.clone();
    forged.signature = bootstrap::sign(message(&good), &BigUint::from(4243u64), "forged");
    assert_eq!(check(&forged, &signer), Err("fee schedule signature"));
    let mut high = good.clone();
    let s = BigUint::from_bytes_be(&high.signature[32..]);
    high.signature[32..].copy_from_slice(&certificate::scalar_bytes(&(p256::n() - s)));
    assert_eq!(check(&high, &signer), Err("fee schedule signature"));
}

/// G5: the native matrix-accumulator decision is exact. A claim on the
/// installed polynomial holds; a false value, a short late output, or a
/// claim folded on another polynomial (a template's toy predecessors) does not.
#[test]
fn native_matrix_accumulator_decision_is_exact() {
    use crate::deferral::{Kind, MatPoly};
    let kind = |offset: u64| Kind::<Fp> {
        sx: 3,
        sz: 4,
        deg: (0, 0),
        eval: Box::new(move |_: &[Fp], rx: &[Fp], ry: &[Fp]| {
            (
                rx.iter().copied().sum::<Fp>() + Fp::from(offset),
                ry.iter().copied().sum(),
                rx[0] * ry[1],
            )
        }),
        eval_many: None,
    };
    let installed = MatPoly::with_min_vec(vec![kind(0), kind(1), kind(2), kind(3)], 3, 4);
    let template = MatPoly::with_min_vec(vec![kind(5), kind(6), kind(7), kind(8)], 3, 4);
    let mut rng = crate::fs::SpikeRng::new(b"native matrix accumulator decision");
    let point: Vec<Fp> = (0..installed.n()).map(|_| rng.scalar()).collect();
    let mut late = point.clone();
    late.push(installed.eval(&point));
    late.extend([Fp::from(11), Fp::from(12)]); // the generator claim follows
    assert!(engine::matrix_accumulator(&installed, &late));
    let mut wrong = late.clone();
    wrong[point.len()] += Fp::ONE;
    assert!(!engine::matrix_accumulator(&installed, &wrong));
    assert!(!engine::matrix_accumulator(
        &installed,
        &late[..point.len()]
    ));
    let mut toy = point.clone();
    toy.push(template.eval(&point));
    assert!(engine::matrix_accumulator(&template, &toy));
    assert!(!engine::matrix_accumulator(&installed, &toy));
    for (message, refusal) in [
        ("native P matrix accumulator", true),
        ("native S inherited generator accumulator", true),
        ("primary linked proof", false),
    ] {
        assert_eq!(
            engine::is_accumulator_refusal(&Error::Rejected(message.into())),
            refusal
        );
    }
}

#[test]
fn retained_operation_codec_preserves_exact_originals() {
    let chain = fixture();
    let operations = [
        ExactOperationWitness::Bootstrap(Box::new(BootstrapWitness {
            scheme: chain.sender_context.root.scheme,
            root: chain.sender_context.root.root.clone(),
            relation: chain.sender_context.relation,
            input: chain.sender_bootstrap.clone(),
        })),
        ExactOperationWitness::Load {
            step: Box::new(chain.load.clone()),
            receipt_original: Arc::from([1u8, 2, 3]),
            finality_original: Arc::from([4u8, 5, 6]),
        },
        ExactOperationWitness::Send(Box::new(chain.send.clone())),
    ];
    for operation in operations {
        let bytes = operation.encode().unwrap();
        let restored = ExactOperationWitness::decode(&bytes).unwrap();
        assert_eq!(restored.encode().unwrap(), bytes);
        for n in [0, 1, bytes.len() / 2, bytes.len() - 1] {
            assert!(ExactOperationWitness::decode(&bytes[..n]).is_err());
        }
        let mut extra = bytes;
        extra.push(0);
        assert!(ExactOperationWitness::decode(&extra).is_err());
    }
}

#[test]
fn native_controls_off_needs_no_clock_but_cannot_choose_time() {
    let chain = fixture();
    let engine = NativeEngine::default();
    let mut send = chain.send;
    // The arithmetic fixture permits an arbitrary conservative upper bound;
    // Native's controls-off path derives the exact floor-only interval.
    send.upper = send.lower;
    assert!(
        engine
            .check_send_time(&ExactOperationWitness::Send(Box::new(send.clone())), false)
            .is_ok()
    );
    send.upper += 1;
    assert!(
        engine
            .check_send_time(&ExactOperationWitness::Send(Box::new(send)), false)
            .is_err()
    );
}

#[test]
fn selected_signature_cannot_move_to_another_frozen_capsule() {
    let chain = fixture();
    let c = &chain.send.credential;
    let mut body = chain.send.receipt.body;
    body[274..306].fill(0);
    let capsule = RecoveryCapsule {
        frame: CapsuleFrame {
            version: RECOVERY_CAPSULE_VERSION,
            parameters_id: chain.sender_context.relation,
            manifest_digest: [8; 32],
            scheme: c.body[2..34].try_into().unwrap(),
            asset: c.body[34..66].try_into().unwrap(),
            wallet: c.body[66..98].try_into().unwrap(),
            payment_key: c.body[130..195].to_vec(),
            kind: 3,
            sequence: u128::from_le_bytes(body[98..114].try_into().unwrap()),
            predecessor: body[146..178].try_into().unwrap(),
            predecessor_capsule_digest: [9; 32],
            successor: body[178..210].try_into().unwrap(),
            operation_id: body[114..146].try_into().unwrap(),
            proof_digest: body[242..274].try_into().unwrap(),
            enrollment_marker: None,
            receipt_body: body.to_vec(),
            recovery: vec![1, 2, 3],
        },
    };
    let encoded = capsule.encode().unwrap();
    let decoded = RecoveryCapsule::decode(&encoded, capsule.scheme()).unwrap();
    assert_eq!(encoded, decoded.encode().unwrap());
    body[274..306].copy_from_slice(&RecoveryCapsule::frame_digest(&encoded));
    let head = VerifiedWalletHead {
        parameters_id: chain.sender_context.relation,
        manifest_digest: [8; 32],
        profile: [Fp::ZERO; 64],
        pair: Arc::from([]),
        credential: c.clone(),
        certificate: chain.sender_bootstrap.certificate.clone(),
        custody_digest: [9; 32],
        enrollment_marker: None,
    };
    let mut prepared = PreparedOperation {
        manifest_digest: [8; 32],
        parameters_id: chain.sender_context.relation,
        capsule,
        witness: Arc::new(ExactOperationWitness::Send(Box::new(chain.send.clone()))),
        receipt_body: body,
        receive: None,
        predecessor: head,
    };
    let signature = bootstrap::sign(
        field(&prepared.signing_message()).unwrap(),
        &chain.sender_key.d,
        "native hardware boundary software test only",
    );
    assert!(HardwareAdvanceReceipt::verify_selected(&prepared, &body, signature).is_ok());
    let mut other = body;
    other[114] ^= 1;
    assert!(HardwareAdvanceReceipt::verify_selected(&prepared, &other, signature).is_err());
    let mut bad = signature;
    bad[0] ^= 1;
    assert!(HardwareAdvanceReceipt::verify_selected(&prepared, &body, bad).is_err());
    prepared.capsule.frame.recovery.push(4);
    let changed = RecoveryCapsule::frame_digest(&prepared.capsule.encode().unwrap());
    prepared.receipt_body[274..306].copy_from_slice(&changed);
    assert!(
        HardwareAdvanceReceipt::verify_selected(&prepared, prepared.receipt_body(), signature)
            .is_err()
    );
}

#[test]
#[ignore = "requires exact signatures exported by isolated physical hardware fixture apps"]
fn physical_hardware_signatures_verify_with_native_p256() {
    let paths =
        std::env::var("KAGEMUSHA_SPARTAN_HARDWARE_VECTORS").expect("comma-separated vector paths");
    for path in paths.split(',') {
        let bytes = std::fs::read(path).unwrap();
        assert_eq!(bytes.len(), 161);
        let q = point(&bytes[..65]).unwrap();
        let mut message: [u8; 32] = bytes[65..97].try_into().unwrap();
        let signature = p256::Signature {
            r: BigUint::from_bytes_be(&bytes[97..129]),
            s: BigUint::from_bytes_be(&bytes[129..161]),
        };
        assert!(signature.s <= p256::n() >> 1);
        let z = BigUint::from_bytes_be(&p256::sha256_one_block(&message));
        assert!(
            p256::verify(&q, &z, &signature),
            "physical signature: {path}"
        );
        message[0] ^= 1;
        let changed = BigUint::from_bytes_be(&p256::sha256_one_block(&message));
        assert!(
            !p256::verify(&q, &changed, &signature),
            "exact hardware message binding: {path}"
        );
    }
}

/// A minimal installable public program (`a * factor = b`) with the monetary
/// IO ABI, for bundle-level manifest tests.
fn tiny_program<F: crate::fs::Scalar + zeroize::Zeroize>(primary: bool, factor: u64) -> Vec<u8> {
    use crate::uniform::{Array, UniformCircuit, constant, own};
    let mut w = crate::full::Wiring::default();
    let f = w.family(0, 1);
    w.push(Array {
        name: "manifest test relation",
        family: f,
        rho: 0,
        r_count: 1,
        fixed: false,
        lookups: vec![],
        widths: vec![255, 255],
        rows: vec![[own(0, 1), constant(vec![F::from(factor)]), own(1, 1)]],
        values: vec![vec![F::from(7)], vec![F::from(7 * factor)]],
    });
    for slot in if primary {
        vec![124, 125, 126, 127]
    } else {
        vec![124, 126]
    } {
        let a = w.push(Array {
            name: "manifest test link slot",
            family: f,
            rho: 0,
            r_count: 1,
            fixed: false,
            lookups: vec![],
            widths: vec![255],
            rows: vec![],
            values: vec![vec![F::ZERO]],
        });
        w.io_arrays.push((a, slot));
        w.link.io.push((a, slot));
    }
    w.link.io_values = Some(Box::new(move |_, _| {
        vec![F::ZERO; if primary { 4 } else { 2 }]
    }));
    let (a, p, n, t, c) = w.finish();
    let (circuit, _, _) = UniformCircuit::build_with_copies(
        if primary { "P-uniform" } else { "S-uniform" },
        a,
        p,
        n,
        t,
        c,
        if primary { 12 } else { 10 },
        64,
        b"manifest test program",
    );
    circuit.export_program().unwrap()
}

/// The retired v1 layout is refused by a typed error, and an isolated fixture
/// can re-issue a root-signed v1 manifest for its exact bundle in the current
/// format, which then authenticates with its pins and profile intact.
#[test]
fn retired_manifest_is_refused_and_fixture_reissue_authenticates() {
    let scheme = [7; 32];
    let primary: [Vec<u8>; 4] = std::array::from_fn(|k| tiny_program::<Fp>(true, 3 + k as u64));
    let secondary: [Vec<u8>; 4] =
        std::array::from_fn(|k| tiny_program::<iroha_pasta::Fq>(false, 3 + k as u64));
    let digest_p = |b: &[u8]| {
        crate::uniform::UniformCircuit::<Fp>::import_program(b, true)
            .unwrap()
            .program_digest()
    };
    let digest_s = |b: &[u8]| {
        crate::uniform::UniformCircuit::<iroha_pasta::Fq>::import_program(b, false)
            .unwrap()
            .program_digest()
    };
    let retired = engine::RetiredManifestV1 {
        version: 1,
        scheme,
        relation: [8; 32],
        primary: std::array::from_fn(|k| digest_p(&primary[k])),
        secondary: std::array::from_fn(|k| digest_s(&secondary[k])),
        dimensions_p: [24, 25],
        dimensions_s: [24, 24],
        finality_network: "test-network".into(),
        finality_chain: "test-chain".into(),
    };
    let bundle = ProgramBundle {
        version: 1,
        primary,
        secondary,
    }
    .encode()
    .unwrap();
    let retired_bytes = encode(&retired).unwrap();
    let retired_signature = bootstrap::sign(
        crate::payment::byte_digest(u64::from_le_bytes(*b"kgwparm1"), &retired_bytes),
        &root_secret(),
        "retired root-signed manifest",
    );
    assert!(matches!(
        engine::authenticate_manifest(&retired_bytes, &retired_signature, &root()),
        Err(Error::AuthorityMismatch(m)) if m.starts_with("retired v1 parameter manifest")
    ));
    let (bytes, signature) = engine::reissue_fixture_manifest(
        &bundle,
        &retired_bytes,
        retired_signature,
        &engine::TEST_ROOT_SECRET,
        &engine::TEST_ARTIFACT_SECRET,
    )
    .unwrap();
    let (manifest, _) = engine::authenticate_manifest(&bytes, &signature, &root()).unwrap();
    assert_eq!(manifest.version, engine::PARAMETER_MANIFEST_VERSION);
    assert_eq!(
        (
            manifest.scheme,
            manifest.relation,
            manifest.primary,
            manifest.secondary
        ),
        (scheme, [8; 32], retired.primary, retired.secondary)
    );
    assert!(engine::authenticate_profile(&manifest, [12, 10]).is_ok());
    // Only the root's own v1 signature over pins equal to the bundle's.
    let mut wrong = retired_signature;
    wrong[0] ^= 1;
    assert!(
        engine::reissue_fixture_manifest(
            &bundle,
            &retired_bytes,
            wrong,
            &engine::TEST_ROOT_SECRET,
            &engine::TEST_ARTIFACT_SECRET,
        )
        .is_err()
    );
    let mut other = engine::RetiredManifestV1 { ..retired };
    other.primary[2][0] ^= 1;
    let other_bytes = encode(&other).unwrap();
    let other_signature = bootstrap::sign(
        crate::payment::byte_digest(u64::from_le_bytes(*b"kgwparm1"), &other_bytes),
        &root_secret(),
        "retired root-signed manifest",
    );
    assert!(matches!(
        engine::reissue_fixture_manifest(
            &bundle,
            &other_bytes,
            other_signature,
            &engine::TEST_ROOT_SECRET,
            &engine::TEST_ARTIFACT_SECRET,
        ),
        Err(Error::AuthorityMismatch("retired manifest program pins"))
    ));
}

#[test]
fn signed_spartan_manifest_binds_registered_relation_and_genesis() {
    use iroha_data_model::sumeragi_finality::test_fixtures::NativeFinalityFixture;
    let scheme = device_fixture::scheme();
    let genesis = NativeFinalityFixture::start_with_explicit_parameters("portable-native-fixture");
    let id = scheme.scheme_id();
    let mut manifest = test_manifest(id, &engine::test_artifact_certificate(&id));
    manifest.relation = scheme.relation_id;
    manifest.finality_network = genesis.network_id().to_string();
    let accept = |m: &ParameterManifest| {
        let bytes = encode(m).unwrap();
        AuthenticatedParameters::authenticated_manifest(
            &bytes,
            &engine::test_sign_manifest(&bytes),
            &scheme,
        )
    };
    assert_eq!(accept(&manifest).unwrap().0.relation, scheme.relation_id);
    manifest.relation[0] ^= 1;
    assert!(accept(&manifest).is_err());
    manifest.relation = scheme.relation_id;
    let other_genesis = NativeFinalityFixture::start_at("portable-native-fixture", 17_001);
    assert_ne!(genesis.network_id(), other_genesis.network_id());
    manifest.finality_network = other_genesis.network_id().to_string();
    assert!(accept(&manifest).is_err());
}

#[test]
#[ignore = "requires closed v4 programs and a new isolated fixture output directory"]
fn issue_genesis_bound_fixture_from_closed_public_programs() {
    use iroha_data_model::sumeragi_finality::test_fixtures::NativeFinalityFixture;
    use std::{fs, path::PathBuf};
    let source = PathBuf::from(std::env::var_os("KAGEMUSHA_SPARTAN_DEVICE_ARTIFACTS").unwrap());
    let destination = PathBuf::from(std::env::var_os("KAGEMUSHA_SPARTAN_TEST_OUTPUT").unwrap());
    let original = fs::read(source.join("parameter-manifest.norito")).unwrap();
    let sig = fs::read(source.join("parameter-signature.bin"))
        .unwrap()
        .try_into()
        .unwrap();
    let (mut manifest, _) = engine::authenticate_manifest(&original, &sig, &root()).unwrap();
    let fixture = NativeFinalityFixture::start_with_explicit_parameters("portable-native-fixture");
    let scheme = device_fixture::scheme();
    assert_eq!(manifest.finality_network, fixture.network_id().to_string());
    assert_eq!(manifest.relation, scheme.relation_id);
    manifest.scheme = scheme.scheme_id();
    let cert = engine::test_artifact_certificate(&manifest.scheme);
    manifest.signer_certificate = [&cert.body[..], &cert.signature[..]].concat();
    let bytes = encode(&manifest).unwrap();
    let signature = engine::test_sign_manifest(&bytes);
    let bundle = fs::read(source.join("program-bundle.norito")).unwrap();
    let parameters = device_fixture::install(&bundle, &bytes, signature).unwrap();
    assert_eq!(parameters.id(), &scheme.relation_id);
    // Optional public World source for the independent carrier publisher.
    if let Some(path) = std::env::var_os("KAGEMUSHA_SPARTAN_WORLD_EXPORT") {
        use iroha_data_model::isi::kagemusha_wallet::spartan::InstallKagemushaSpartanProgramsV1;
        use iroha_data_model::kagemusha::{
            KagemushaDevicePublicKeyV1, KagemushaWalletSignerCertificateBodyV1,
            KagemushaWalletSignerCertificateV1, KagemushaWalletSignerOutputV1, KagemushaWalletSignerRoleV1,
        };
        use std::io::Write as _;
        let path = std::path::PathBuf::from(path);
        fs::create_dir(&path).unwrap();
        let fixture = crate::bootstrap::fixture_vector("credential_body_468");
        let install = InstallKagemushaSpartanProgramsV1 {
            scheme: scheme.scheme_id(), asset: fixture[34..66].try_into().unwrap(),
            manifest_digest: *parameters.manifest_digest(), manifest: bytes.clone(),
            signature, programs: bundle.clone(),
        };
        let _layout = norito::core::WireLayoutGuard::enter(norito::core::WireLayout::Fixed);
        let certificate_frame = |raw: &crate::certificate::Certificate, role: KagemushaWalletSignerRoleV1| {
            let body = KagemushaWalletSignerCertificateBodyV1 {
                version: 1,
                scheme_id: scheme.scheme_id(),
                role,
                key: KagemushaDevicePublicKeyV1::from_sec1_bytes(&raw.body[35..100]).unwrap(),
                serial: u64::from_le_bytes(raw.body[100..108].try_into().unwrap()),
            };
            assert_eq!(body.transcript(), raw.body);
            let certificate = KagemushaWalletSignerCertificateV1::sign(
                body, &scheme, KagemushaWalletSignerOutputV1::Raw(raw.signature),
            ).unwrap();
            assert_eq!(certificate.signature.as_ref(), raw.signature);
            certificate.to_canonical_bytes().unwrap()
        };
        for (name, original) in [
            ("registered-scheme.norito", scheme.to_canonical_bytes().unwrap()),
            ("selected-world-install.norito", norito::to_bytes(&install).unwrap()),
            ("artifact-certificate.norito", certificate_frame(&cert, KagemushaWalletSignerRoleV1::Artifact)),
            ("enrollment-certificate.norito", certificate_frame(&device_fixture::enrollment_certificate(), KagemushaWalletSignerRoleV1::Enrollment)),
        ] {
            let mut f = fs::File::create_new(path.join(name)).unwrap();
            f.write_all(&original).unwrap();
            let mut permissions = f.metadata().unwrap().permissions();
            permissions.set_readonly(true);
            f.set_permissions(permissions).unwrap();
            f.sync_all().unwrap();
        }
    }
    // No credential, balance, authorization or custody data is transferred.
    fs::create_dir(&destination).unwrap();
    fs::write(destination.join("program-bundle.norito"), bundle).unwrap();
    fs::write(destination.join("parameter-manifest.norito"), bytes).unwrap();
    fs::write(destination.join("parameter-signature.bin"), signature).unwrap();
    eprintln!(
        "Eight installed programs; signed fixture manifest digest {:02x?}; local fixture network {}",
        parameters.manifest_digest(),
        fixture.network_id()
    );
}
