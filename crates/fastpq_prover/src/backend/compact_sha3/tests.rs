//! Complete canonical prefix/body, exact cache and SHA3/SHAKE separation tests.
use super::*;
#[test]
fn cached_prefix_matches_complete_input_and_binds_every_public_frame_field() {
    let context = Context::new(b"complete immutable statement").unwrap();
    let longer = Context::new(b"complete immutable statement!").unwrap();
    assert!(context.same_attempt(&context.clone()));
    assert!(!context.same_attempt(&Context::new(b"complete immutable statement").unwrap()));
    assert_eq!(
        longer.maximum_retained_payload_bytes().unwrap(),
        context.maximum_retained_payload_bytes().unwrap() + 1
    );
    for size in [0, 1, 7, 8, 96, 2408, 4096] {
        let payload = vec![37; size];
        let frame = context.frame(1, 1, 0, 0, 17, 32, BodyFields::One(&payload));
        let body = norito::encode_canonical(&frame).unwrap();
        let prepared = context.prepare_hash_frame(&frame).unwrap();
        assert_eq!(&*prepared.encoded, body);
        let mut h = Sha3_256V1::new();
        h.update(&context.prefix.encoded);
        h.update(&body);
        let expected = h.finalize();
        assert_eq!(context.hash_frame(&frame).unwrap(), expected);
        assert_eq!(prepared.hash_cpu(), expected);
        assert_ne!(longer.hash_frame(&frame).unwrap(), expected);
        for (kind, oracle, round, level, index, width) in [
            (2, 1, 0, 0, 17, 32),
            (1, 2, 0, 0, 17, 32),
            (1, 1, 1, 0, 17, 32),
            (1, 1, 0, 1, 17, 32),
            (1, 1, 0, 0, 18, 32),
            (1, 1, 0, 0, 17, 31),
        ] {
            assert_ne!(
                context
                    .hash_frame(&context.frame(
                        kind,
                        oracle,
                        round,
                        level,
                        index,
                        width,
                        BodyFields::One(&payload)
                    ))
                    .unwrap(),
                expected
            );
        }
    }
    let oversized = vec![0; MAX_PREPARED_HASH_FRAME_BYTES];
    assert!(
        context
            .prepare_hash_frame(&context.frame(1, 1, 0, 0, 0, 32, BodyFields::One(&oversized)))
            .is_err()
    );
    assert!(Context::new(b"").is_err());
    assert!(Context::new(&vec![0; MAX_CONTEXT_BYTES + 1]).is_err());
}
#[test]
fn whole_raw_tape_matches_one_shot_shake_without_context_substitution() {
    let context = Context::new(&vec![91; MAX_CONTEXT_BYTES]).unwrap();
    for ordinal in 1..=10 {
        let round = RawTapeRoundV1::new(ordinal).unwrap();
        let frame = context.frame(
            4,
            0,
            ordinal,
            0,
            0,
            round.tape_bytes(),
            BodyFields::One(&[0xff; 32]),
        );
        let body = norito::encode_canonical(&frame).unwrap();
        let actual = context.tape(round, &body).unwrap();
        let mut xof = Shake256V1::new();
        xof.update(&context.prefix.encoded);
        xof.update(&body);
        let mut expected = vec![0; round.tape_bytes()];
        xof.finalize().read(&mut expected);
        assert_eq!(actual.as_bytes(), expected);
        assert_ne!(
            &actual.as_bytes()[..32],
            context.hash_frame(&frame).unwrap().as_bytes()
        );
    }
}

#[test]
fn counted_prefix_and_body_permutations_match_one_shot_rate_geometry() {
    let context = Context::new(b"Keccak rate boundary work ledger").unwrap();
    let prefix = context.prefix.encoded.len();
    assert_eq!(context.prefix_permutations(), 2 * (prefix / 136));
    for body in [0, 1, 135, 136, 137, 8192, 29584] {
        assert_eq!(
            prefix / 136 + context.body_permutations(body).unwrap(),
            (prefix + body) / 136 + 1
        );
    }
    assert!(context.body_permutations(usize::MAX).is_err());
}
