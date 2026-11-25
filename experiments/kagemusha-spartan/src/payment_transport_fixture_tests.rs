//! Read-only old Payment vector for the new reversible transport. The old peer
//! schema is parsed here only. The original proof and signed originals remain exact.
use super::*;

pub(super) fn check(profile: &Profile<'_>, old: &[u8]) -> Vec<u8> {
    assert_eq!(
        &old[..12],
        &[
            [b'I', b'P', b'M', b'1'].as_slice(),
            &[1, 0, 0x80, 1, 3, 0, 0, 2]
        ]
        .concat()
    );
    assert!(peer_decode(old).is_err());
    let payload = &old[HEADER_BYTES..];
    assert_eq!(
        &old[20..52],
        &hash(&[
            PAYLOAD_DOMAIN,
            &PEER_PROFILE.to_be_bytes(),
            &[3],
            &2u16.to_be_bytes(),
            payload
        ])
    );
    assert_eq!(&old[52..84], &hash(&[MESSAGE_DOMAIN, &old[..52], payload]));
    assert_eq!(&payload[..8], PAYMENT_MAGIC);
    let len = u32::from_be_bytes(payload[8..12].try_into().unwrap()) as usize;
    let old_envelope = &payload[16..16 + len];
    let companion = &payload[16 + len..];
    let view = norito::core::from_bytes_view(old_envelope).unwrap();
    let _flags = norito::core::DecodeFlagsGuard::enter(norito::core::V1_LAYOUT_FLAGS);
    let (old_fields, n) = norito::core::with_decode_limits(
        norito::canonical_decode_limits(old_envelope.len()),
        || norito::core::decode_field_canonical::<Envelope>(view.as_bytes()),
    )
    .unwrap();
    assert_eq!(n, view.as_bytes().len());
    assert_eq!(old_fields.version, 1);
    let (payer, certificate) =
        companion_restore(profile, &read_companion(companion).unwrap()).unwrap();
    let statement_init =
        payment_words::pack(&old_fields.statement, &statement_positions()).unwrap();
    let (statement, request) =
        statement_restore(profile, &payer, &statement_init, &old_fields.request).unwrap();
    let originals = Originals {
        receipt: receipt_restore(&old_fields.receipt, &payer, &statement).unwrap(),
        payer,
        certificate,
        request,
        statement,
        policy_epoch: old_fields.policy_epoch,
        pending_root: field(&old_fields.pending_root).unwrap(),
        credit_root: field(&old_fields.credit_root).unwrap(),
    };
    let expected = payment::digests(
        &originals.request,
        &originals.payer,
        &originals.statement,
        &originals.receipt,
        &old_fields.pair,
    )
    .unwrap();
    let bytes = test_reframe_originals(profile, &originals, old_fields.pair.clone()).unwrap();
    let strict = decode(profile, &bytes).expect("genuine re-encoding passes strict pair decoding");
    assert_eq!(strict.pair_bytes, old_fields.pair);
    let planned = admit_send_profile(profile.pair).unwrap();
    assert_eq!(planned.peer, bytes.len());
    assert_eq!(planned.envelope, strict.size.envelope);
    assert_eq!(planned.pair, strict.size.pair);
    assert_eq!(planned.companion, strict.size.companion);
    let result = decode_candidate(profile, &bytes).unwrap();
    assert_eq!(result.pair_bytes, old_fields.pair);
    assert_eq!(result.digests.proof, expected.proof);
    assert_eq!(result.originals.statement, originals.statement);
    assert_eq!(result.originals.receipt.body, originals.receipt.body);
    assert_eq!(
        result.originals.receipt.signature,
        originals.receipt.signature
    );
    assert_eq!(result.originals.request.body, originals.request.body);
    assert_eq!(
        result.originals.request.signature,
        originals.request.signature
    );
    let (eb, cb) = peer_decode(&bytes).unwrap();
    assert_eq!(cb, companion);
    let e = read_envelope(eb).unwrap();
    eprintln!(
        "Historical genuine Payment: {} -> {} complete IPM1 bytes; statement {} -> {}; pair {} -> {}; companion {} exact",
        old.len(),
        bytes.len(),
        old_fields.statement.len(),
        e.statement.len(),
        old_fields.pair.len(),
        e.pair.len(),
        cb.len()
    );
    assert!(old.len() - bytes.len() > 850);
    let (_, positions) = profile.pair.payment_generator_words().unwrap();
    // Each raw 32-byte generator word expands this envelope by exactly 16 bytes.
    let maximum = (MAX_ENVELOPE_BYTES - eb.len()) / 16;
    assert!(maximum > 0 && maximum < positions.len());
    let mut malformed = old_fields.pair;
    for &(at, _) in &positions[..maximum] {
        malformed[at..at + 32].fill(0xff);
    }
    let candidate = test_reframe_originals(profile, &originals, malformed.clone()).unwrap();
    assert_eq!(
        peer_decode(&candidate).unwrap().0.len(),
        eb.len() + 16 * maximum
    );
    assert_eq!(
        decode_candidate(profile, &candidate).unwrap().pair_bytes,
        malformed
    );
    assert!(decode(profile, &candidate).is_err());
    let next = positions[maximum].0;
    malformed[next..next + 32].fill(0xff);
    assert!(test_reframe_originals(profile, &originals, malformed).is_err());
    eprintln!(
        "Genuine envelope {} B; {maximum} raw generator words admitted; {} refused",
        eb.len(),
        maximum + 1
    );
    bytes
}

/// A transition proof produced by the current v4 relation can be measured after
/// the carrier is finalized without regenerating a witness or signing again.
#[test]
#[ignore = "requires generated v4 public programs, signed fixture installation and its genuine Payment"]
fn signed_v4_payment_verifies_within_complete_peer_limits() {
    use crate::native::{Engine, NativeEngine, ParameterManifest, device_fixture};
    use crate::uniform::UniformCircuit;
    let dir = std::path::PathBuf::from(std::env::var_os("KAGEMUSHA_SPARTAN_V4_PROGRAMS").unwrap());
    let install = dir.join("native-device");
    let manifest_bytes = std::fs::read(install.join("parameter-manifest.norito")).unwrap();
    let parameters = device_fixture::install(
        &std::fs::read(install.join("program-bundle.norito")).unwrap(),
        &manifest_bytes,
        std::fs::read(install.join("parameter-signature.bin"))
            .unwrap()
            .try_into()
            .unwrap(),
    )
    .unwrap();
    let manifest: ParameterManifest =
        norito::decode_from_bytes_with_limits(&manifest_bytes, limits(manifest_bytes.len()))
            .unwrap();
    let primary = UniformCircuit::<Fp>::import_program(
        &std::fs::read(dir.join("research-kind-1-primary.norito")).unwrap(),
        true,
    )
    .unwrap();
    let secondary = UniformCircuit::<Fq>::import_program(
        &std::fs::read(dir.join("research-kind-1-secondary.norito")).unwrap(),
        false,
    )
    .unwrap();
    assert_eq!(
        primary.program_digest(),
        manifest.primary[crate::full::SEND]
    );
    assert_eq!(
        secondary.program_digest(),
        manifest.secondary[crate::full::SEND]
    );
    let pair = TrustedProfile::from_parameters(
        manifest.relation,
        primary.shape,
        &primary,
        secondary.shape,
        &secondary,
        true,
    )
    .unwrap();
    let profile = Profile {
        pair: &pair,
        scheme: manifest.scheme,
        relation: manifest.relation,
    };
    let original = std::fs::read(dir.join("research-payment.ipm1")).unwrap();
    let bytes = if original.get(10..12) == Some(&PEER_SCHEMA.to_be_bytes()) {
        original
    } else {
        check(&profile, &original)
    };
    NativeEngine::default()
        .verify_payment(&bytes, &parameters)
        .unwrap();
    let value = decode(&profile, &bytes).unwrap();
    eprintln!(
        "Fresh signed v4 Payment: complete IPM1 {} B, envelope {} B, companion {} B, on-wire pair {} B, original proof {} B",
        bytes.len(),
        value.size.envelope,
        value.size.companion,
        value.size.pair,
        value.pair_bytes.len()
    );
    assert!(bytes.len() <= QR_BUDGET);
    if let Some(path) = std::env::var_os("KAGEMUSHA_SPARTAN_V4_PAYMENT_OUTPUT") {
        use std::io::Write;
        match std::fs::File::create_new(&path) {
            Ok(mut f) => f.write_all(&bytes).unwrap(),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                assert_eq!(std::fs::read(path).unwrap(), bytes)
            }
            Err(e) => panic!("Payment fixture output: {e}"),
        }
    }
}
