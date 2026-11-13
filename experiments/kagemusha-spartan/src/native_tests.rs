use super::*;
use ff::{Field, PrimeField};

fn fixture() -> crate::chain_fixture::Chain {
    crate::chain_fixture::new(&crate::ipa::Gens::<iroha_pasta::Eq>::new(4096))
}

#[test]
fn parameter_manifest_requires_exact_root_signature() {
    let secret = BigUint::from(12345u64);
    let root = p256::mul(&p256::generator(), &secret).unwrap();
    let manifest = ParameterManifest {
        version: 1,
        scheme: [7; 32],
        relation: [8; 32],
        primary: [[1; 32]; 4],
        secondary: [[2; 32]; 4],
        dimensions_p: [28, 26],
        dimensions_s: [27, 25],
        finality_network: "test-network".into(),
        finality_chain: "test-chain".into(),
    };
    let bytes = encode(&manifest).unwrap();
    let message = crate::payment::byte_digest(u64::from_le_bytes(*b"kgwparm1"), &bytes);
    let signature = bootstrap::sign(message, &secret, "native manifest test");
    assert!(engine::authenticate_manifest(&bytes, &signature, &root).is_ok());
    let mut changed = manifest;
    changed.primary[0][0] ^= 1;
    assert!(engine::authenticate_manifest(&encode(&changed).unwrap(), &signature, &root).is_err());
    changed.primary[0][0] ^= 1;
    changed.dimensions_p[0] -= 1;
    assert!(engine::authenticate_manifest(&encode(&changed).unwrap(), &signature, &root).is_err());
    let wrong = p256::mul(&p256::generator(), &BigUint::from(12346u64)).unwrap();
    assert!(engine::authenticate_manifest(&bytes, &signature, &wrong).is_err());
    let mut extra = bytes.clone();
    extra.push(0);
    assert!(engine::authenticate_manifest(&extra, &signature, &root).is_err());
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
            version: 1,
            parameters_id: chain.sender_context.relation,
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
        profile: [Fp::ZERO; 64],
        pair: Arc::from([]),
        credential: c.clone(),
        certificate: chain.sender_bootstrap.certificate.clone(),
        custody_digest: [9; 32],
        enrollment_marker: None,
    };
    let mut prepared = PreparedOperation {
        parameters_id: chain.sender_context.relation,
        capsule,
        witness: Arc::new(ExactOperationWitness::Send(Box::new(chain.send.clone()))),
        receipt_body: body,
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
