//! Primitive FI read-cut joining through the existing real synthetic Ed/BLS/World fixture.
//! No FI approved release, customer/session, installed authority or query permission is minted.
use super::*;

fn produce(
    f: &Fixture,
    request: &ParticipantEnrollmentRequestV1<'_>,
) -> Result<VerifiedParticipantEnrollmentRequestV1> {
    let challenge = EnrollmentWalletReadChallengeV1::for_request(request)?;
    let nodes = Fixture::nodes();
    let statements = f.statements(challenge.bytes(), &nodes);
    let signature = Signature::new(f.signer.private_key(), &request.signing_message()?);
    authenticate_fi_current_request_cut(
        challenge,
        request,
        &signature,
        &f.native.verifier(),
        f.native.latest(),
        f.world.schema_hash,
        &f.world,
        (f.signatory.clone(), f.value.clone()),
        (f.wallet.clone(), f.value.clone()),
        &nodes,
        &statements,
    )
}

#[test]
fn fi_receiving_current_cut_retains_real_signature_original_body_and_exact_native_subject() {
    let f = Fixture::new();
    let network = f.native.network_id();
    let target = valid_target();
    let request = request(&f, &network, &target, b"{ \"actual\":\"original\" }\n");
    let verified = produce(&f, &request).unwrap();
    verified.verify_original_body(request.body).unwrap();
    assert_eq!(verified.actor_id(), request.actor_id);
    assert_eq!(verified.namespace(), request.authentication_namespace);
    assert_eq!(verified.signatory(), &f.signatory);
    assert_eq!(verified.wallet(), &f.wallet);
    assert_eq!(verified.target(), target.as_str());
    assert_eq!(verified.operation(), request.operation);
    verified
        .verify_fi_owned_selection(
            &network,
            &Fixture::nodes(),
            f.world.schema_hash,
            &f.native.verifier(),
            f.native.latest().height(),
        )
        .unwrap();
    assert!(verified.verify_original_body(b"changed").is_err());
}

#[test]
fn fi_receiving_current_cut_refuses_twelve_well_formed_original_substitutions() {
    let f = Fixture::new();
    let network = f.native.network_id();
    let target = valid_target();
    let original = request(&f, &network, &target, b"original");
    for variant in 0..12 {
        // Establish genuine current evidence through the exact production join before each
        // mutation; none of these refusals can be explained by an invalid base fixture.
        produce(&f, &original).unwrap();
        let challenge = EnrollmentWalletReadChallengeV1::for_request(&original).unwrap();
        let mut nodes = Fixture::nodes();
        let mut statements = f.statements(challenge.bytes(), &nodes);
        let mut world = f.world.clone();
        let mut s = (f.signatory.clone(), f.value.clone());
        let mut w = (f.wallet.clone(), f.value.clone());
        let mut offered = original;
        let mut signature =
            Signature::new(f.signer.private_key(), &original.signing_message().unwrap());
        match variant {
            0 => s.0 = f.wallet.clone(),
            1 => w.0 = f.signatory.clone(),
            2 => {
                world.entries.pop().unwrap();
            }
            3 => world.schema_hash = Hash::new(b"foreign complete World schema"),
            4 => nodes[3] = nodes[0].clone(),
            5 => nodes[0].build_fingerprint = Hash::new(b"different actual executable"),
            6 => statements[3] = statements[0].clone(),
            7 => offered.body = b"different original body",
            8 => offered.session_sha256 = [8; 32],
            9 => {
                let foreign = KeyPair::from_seed(vec![88; 32], Algorithm::Ed25519);
                signature =
                    Signature::new(foreign.private_key(), &original.signing_message().unwrap());
            }
            10 => {
                s.1.metadata_mut().insert(
                    "changed_original".parse().unwrap(),
                    iroha_primitives::json::Json::new(1_u64),
                );
            }
            11 => {
                w.1.metadata_mut().insert(
                    "changed_original".parse().unwrap(),
                    iroha_primitives::json::Json::new(1_u64),
                );
            }
            _ => unreachable!(),
        }
        assert!(
            authenticate_fi_current_request_cut(
                challenge,
                &offered,
                &signature,
                &f.native.verifier(),
                f.native.latest(),
                f.world.schema_hash,
                &world,
                s,
                w,
                &nodes,
                &statements,
            )
            .is_err(),
            "variant {variant}"
        );
    }
}
