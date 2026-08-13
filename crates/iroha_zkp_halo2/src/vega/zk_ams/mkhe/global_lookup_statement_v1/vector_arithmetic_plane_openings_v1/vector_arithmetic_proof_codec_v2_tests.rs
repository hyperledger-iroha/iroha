use super::*;

const VALID_POINT_V2: [u8; POINT_BYTES_V2] = [
    0x80, 0x25, 0xa4, 0xe3, 0x12, 0x8f, 0x04, 0x2d, 0x72, 0x8e, 0x58, 0xb7, 0xe0, 0x9a, 0x51, 0xb7,
    0x25, 0x85, 0xbe, 0x44, 0x35, 0xf4, 0xe9, 0x4a, 0xac, 0x85, 0x17, 0xf2, 0xe1, 0x58, 0xb3, 0xea,
    0xe6,
];

fn decode_32_v2(encoded: &str) -> [u8; 32] {
    hex::decode(encoded)
        .expect("hex fixture")
        .try_into()
        .expect("32-byte fixture")
}

fn independent_manifest_oracle_v2() -> [u8; 32] {
    let domain = b"iroha.zk-ams.v1.phase23.global-lookup.vector-arithmetic-proof.manifest\0";
    let languages: [&[u8]; 5] = [
        b"for-v=0..16383;q3[v]=sum_g=0..343(kappa^g*(bD_g[v]*(bD_g[v]-1)+delta*bS_g[v]*(bS_g[v]-1)+delta^2*bD_g[v]*bS_g[v]));q5[v]=sum_g=0..343(kappa^g*(sum_h=0..17(delta^h*beta_g,h[v]*(beta_g,h[v]-1))+delta^18*(m_g[v]-bD_g[v]*beta_g,16[v])+delta^19*(beta_g,17[v]-beta_g,16[v]+m_g[v])));q8[v]=sum_u=0..1031(kappa^u*((x_u[v]+n_u[v])*n_u[v]))",
        b"logical-plane-to-commitment-session-physical-role:bD[0..343]->[12040..12383];bS[344..687]->[12384..12727];beta[688..6879]->[18576..24767];m[6880..7223]->[24768..25111];x[7224..8255]->[25112..26143];n[8256..9287]->[26144..27175];q3,q5,q8[9288..9290]->[71106..71108];9291-logical-planes;no-inverse-roles",
        b"proof-order=S3,S5,S8;commitment-order=S3=(bD[0..343],bS[0..343],q3),S5=(bD[0..343],beta[group-major][0..17],m[0..343],q5),S8=(x[0..1031],n[0..1031],q8);gate-order=coordinate-v-major;S3-then-group-major-then-(bD-boolean,bS-boolean,bD-times-bS);S5-then-group-major-then-(beta-boolean-h=0..17,bD-times-beta16);S8-then-unit-major-(x-plus-n)-times-n;constraint-order=two-input-links-per-multiplication-in-gate-order-then-one-output-aggregate-per-coordinate;constraint-count=2*actual-gates+16384;padded-gates-have-no-logical-plane-or-extra-aggregate",
        b"one-aggregate-purpose-manifest-shared-by-all-three-envelopes;ZGVA||codec-version:u8=1||flags:u8=0||statement:u8||logP:u8||C:u16be||actual-gates:u32be||core-len:u32be||aggregate-purpose-manifest:[u8;32];exactly-50-bytes;wire=50-byte-envelope||raw-generalized-Bulletproof-core;core-order=(2*C+7)-nonidentity-points,3-canonical-scalars,2*logP-nonidentity-IPA-points,2-canonical-final-scalars;exact-consumption",
        b"current-T256-generalized-Bulletproof-basis-cap=65536-gates;required-padded-gates=(33554432,134217728,33554432);current-eager-backend-cannot-instantiate-any-shape;raising-only-the-cap-is-insufficient;requires-streaming-sparse-tensor-aware-backend-or-new-product-argument;chunking-at-65536-requires-2150-cores-and-at-least-3252950B>709746B-pre-envelope-room;raw-cores=(47515,456319,138331);three-50B-envelopes;known-base=32844686;section=642315;conditional-total=33487001;cap=33554432;margin=67431;about-226.42-bit-simple-union-estimate-is-not-an-accepted-soundness-theorem;all-readiness-gates-zero",
    ];
    let topology = decode_32_v2("3af9a6ad67383c32b06bb5d95a05863b8cb0b3338660177bc2a92e1bbf40b4ab");
    let challenge =
        decode_32_v2("e3730911785cb1e23332ee9a1361810c435f76b93becd54e3b0d189644b32d99");
    let basis = decode_32_v2("bf81c83091a426bbcb2f7518ad3716391810e50b848b38d0c2b3cd96aff9a3f8");
    let mapping = decode_32_v2("689dc6e0841287f2ab74c81366ff64cd5a490faffe0b1d35bb69090dbbe764d9");
    let shapes = [
        (
            0_u8,
            3_u8,
            25_u8,
            689_u16,
            16_908_288_u32,
            16_646_144_u32,
            33_832_960_u32,
            47_515_u32,
            47_565_u32,
        ),
        (
            1,
            5,
            27,
            6_881,
            107_085_824,
            27_131_904,
            214_188_032,
            456_319,
            456_369,
        ),
        (
            2, 8, 25, 2_065, 16_908_288, 16_646_144, 33_832_960, 138_331, 138_381,
        ),
    ];
    let mut hash = Keccak256::new();
    hash.update(domain);
    hash.update(&[1]);
    for digest in [topology, challenge, basis, mapping] {
        hash.update(&digest);
    }
    for language in languages {
        hash.update(&(language.len() as u16).to_be_bytes());
        hash.update(language);
    }
    hash.update(&[3]);
    for (ordinal, statement, log_p, commitments, gates, padding, constraints, core, wire) in shapes
    {
        hash.update(&[ordinal, statement, log_p]);
        hash.update(&commitments.to_be_bytes());
        for value in [gates, padding, constraints, core] {
            hash.update(&value.to_be_bytes());
        }
        hash.update(&50_u16.to_be_bytes());
        hash.update(&wire.to_be_bytes());
    }
    for value in [32_844_686_u32, 642_315, 33_487_001, 33_554_432, 67_431] {
        hash.update(&value.to_be_bytes());
    }
    hash.update(&[0; 12]);
    hash.finalize()
}

fn canonical_wire_v2(ordinal: usize) -> Vec<u8> {
    let shape = proof_shape_v2(ordinal).expect("shape");
    let mut envelope = [0_u8; VECTOR_ARITHMETIC_ENVELOPE_BYTES_V2];
    write_vector_arithmetic_envelope_v2(&mut envelope, ordinal).expect("envelope");
    let mut wire = Vec::with_capacity(shape.wire_bytes as usize);
    wire.extend_from_slice(&envelope);
    for _ in 0..(2 * u32::from(shape.commitment_count) + GENERALIZED_BP_FIXED_POINTS_V2) {
        wire.extend_from_slice(&VALID_POINT_V2);
    }
    wire.extend_from_slice(&[0; 3 * SCALAR_BYTES_V2]);
    for _ in 0..2 * u32::from(shape.log_padded_gates) {
        wire.extend_from_slice(&VALID_POINT_V2);
    }
    wire.extend_from_slice(&[0; 2 * SCALAR_BYTES_V2]);
    assert_eq!(wire.len(), shape.wire_bytes as usize);
    wire
}

#[test]
fn manifest_kat_is_reproduced_by_an_independent_oracle() {
    validate_static_profile_v2().expect("frozen profile");
    assert_eq!(
        hex::encode(plane_mapping_digest_v1().unwrap()),
        "689dc6e0841287f2ab74c81366ff64cd5a490faffe0b1d35bb69090dbbe764d9"
    );
    let digest = vector_arithmetic_manifest_digest_v2().expect("manifest");
    assert_eq!(digest, independent_manifest_oracle_v2());
    assert_eq!(
        hex::encode(digest),
        "108ae021b7519ecdf7f2f917e3dfd10702d7939911ccc24182054f32441c5840"
    );
}

#[test]
fn exact_equations_dense_mapping_and_fixed_order_are_frozen() {
    for literal in [
        b"q3[v]=sum_g=0..343".as_slice(),
        b"delta^2*bD_g[v]*bS_g[v]",
        b"sum_h=0..17(delta^h*beta_g,h[v]*(beta_g,h[v]-1))",
        b"delta^18*(m_g[v]-bD_g[v]*beta_g,16[v])",
        b"delta^19*(beta_g,17[v]-beta_g,16[v]+m_g[v])",
        b"q8[v]=sum_u=0..1031(kappa^u*((x_u[v]+n_u[v])*n_u[v]))",
    ] {
        assert!(
            EQUATION_LANGUAGE_V2
                .windows(literal.len())
                .any(|part| part == literal)
        );
    }
    for literal in [
        b"bD[0..343]->[12040..12383]".as_slice(),
        b"bS[344..687]->[12384..12727]",
        b"beta[688..6879]->[18576..24767]",
        b"m[6880..7223]->[24768..25111]",
        b"x[7224..8255]->[25112..26143]",
        b"n[8256..9287]->[26144..27175]",
        b"q3,q5,q8[9288..9290]->[71106..71108]",
        b"no-inverse-roles",
    ] {
        assert!(
            DENSE_MAPPING_ORDER_LANGUAGE_V2
                .windows(literal.len())
                .any(|part| part == literal)
        );
    }
    assert!(COMMITMENT_GATE_ORDER_LANGUAGE_V2.starts_with(b"proof-order=S3,S5,S8"));
    assert!(
        COMMITMENT_GATE_ORDER_LANGUAGE_V2
            .windows(b"constraint-count=2*actual-gates+16384".len())
            .any(|part| part == b"constraint-count=2*actual-gates+16384")
    );
}

#[test]
fn shapes_core_formula_and_wire_accounting_are_exact() {
    let expected = [
        (
            3, 25, 689, 16_908_288, 33_554_432, 16_646_144, 33_832_960, 47_515, 47_565,
        ),
        (
            5,
            27,
            6_881,
            107_085_824,
            134_217_728,
            27_131_904,
            214_188_032,
            456_319,
            456_369,
        ),
        (
            8, 25, 2_065, 16_908_288, 33_554_432, 16_646_144, 33_832_960, 138_331, 138_381,
        ),
    ];
    let mut total = 0_u32;
    for (ordinal, expected) in expected.into_iter().enumerate() {
        let shape = proof_shape_v2(ordinal).unwrap();
        assert_eq!(
            (
                shape.statement,
                shape.log_padded_gates,
                shape.commitment_count,
                shape.actual_gates,
                shape.padded_gates,
                shape.padding_gates,
                shape.constraint_count,
                shape.core_bytes,
                shape.wire_bytes
            ),
            expected
        );
        assert_eq!(
            shape.constraint_count,
            2 * shape.actual_gates + COORDINATE_COUNT_V2
        );
        assert_eq!(
            shape.core_bytes,
            generalized_bp_core_bytes_v2(shape).unwrap()
        );
        total += shape.wire_bytes;
    }
    assert_eq!(proof_shape_v2(3), Err(VectorArithmeticCodecErrorV2::Order));
    assert_eq!(total, VECTOR_ARITHMETIC_SECTION_WIRE_BYTES_V2);
    assert_eq!(
        KNOWN_BASE_WIRE_BYTES_V2 + total,
        CONDITIONAL_TOTAL_WIRE_BYTES_V2
    );
    assert_eq!(
        RELEASE_WIRE_CAP_BYTES_V2 - CONDITIONAL_TOTAL_WIRE_BYTES_V2,
        67_431
    );
}

#[test]
fn envelopes_are_exact_share_one_manifest_and_reject_mutation() {
    let expected = [
        "5a4756410100031902b1010200000000b99b108ae021b7519ecdf7f2f917e3dfd10702d7939911ccc24182054f32441c5840",
        "5a4756410100051b1ae1066200000006f67f108ae021b7519ecdf7f2f917e3dfd10702d7939911ccc24182054f32441c5840",
        "5a4756410100081908110102000000021c5b108ae021b7519ecdf7f2f917e3dfd10702d7939911ccc24182054f32441c5840",
    ];
    let manifest = vector_arithmetic_manifest_digest_v2().unwrap();
    for (ordinal, expected) in expected.into_iter().enumerate() {
        let mut envelope = [0_u8; VECTOR_ARITHMETIC_ENVELOPE_BYTES_V2];
        write_vector_arithmetic_envelope_v2(&mut envelope, ordinal).unwrap();
        assert_eq!(hex::encode(envelope), expected);
        assert_eq!(&envelope[18..], &manifest);
    }
    assert_eq!(
        write_vector_arithmetic_envelope_v2(&mut [0; VECTOR_ARITHMETIC_ENVELOPE_BYTES_V2], 3),
        Err(VectorArithmeticCodecErrorV2::Order)
    );

    let mut wire = canonical_wire_v2(0);
    for offset in [0, 4, 5, 6, 7, 8, 10, 14, 18, 49] {
        wire[offset] ^= 1;
        assert_eq!(
            borrow_vector_arithmetic_proof_exact_v2(&wire, 0).map(|_| ()),
            Err(VectorArithmeticCodecErrorV2::WireEncoding)
        );
        wire[offset] ^= 1;
    }
}

#[test]
fn borrowed_parser_scans_canonical_syntax_and_fixed_set_order() {
    let s3 = canonical_wire_v2(0);
    let s5 = canonical_wire_v2(1);
    let s8 = canonical_wire_v2(2);
    let set = borrow_vector_arithmetic_proof_set_exact_v2([&s3, &s5, &s8]).unwrap();
    assert_eq!(
        core::array::from_fn(|ordinal| set.proofs[ordinal].shape.statement),
        [3, 5, 8]
    );
    assert_eq!(
        set.proofs[0].envelope,
        &s3[..VECTOR_ARITHMETIC_ENVELOPE_BYTES_V2]
    );
    assert_eq!(set.proofs[1].core.len(), 456_319);
    assert_eq!(
        borrow_vector_arithmetic_proof_set_exact_v2([&s5, &s3, &s8]).map(|_| ()),
        Err(VectorArithmeticCodecErrorV2::WireEncoding)
    );

    let mut bad_point = canonical_wire_v2(0);
    bad_point
        [VECTOR_ARITHMETIC_ENVELOPE_BYTES_V2..VECTOR_ARITHMETIC_ENVELOPE_BYTES_V2 + POINT_BYTES_V2]
        .fill(0);
    assert_eq!(
        borrow_vector_arithmetic_proof_exact_v2(&bad_point, 0).map(|_| ()),
        Err(VectorArithmeticCodecErrorV2::PointEncoding)
    );
    let shape = proof_shape_v2(0).unwrap();
    let scalar_offset = VECTOR_ARITHMETIC_ENVELOPE_BYTES_V2
        + (2 * usize::from(shape.commitment_count) + GENERALIZED_BP_FIXED_POINTS_V2 as usize)
            * POINT_BYTES_V2;
    let mut bad_scalar = canonical_wire_v2(0);
    bad_scalar[scalar_offset..scalar_offset + SCALAR_BYTES_V2].fill(0xff);
    assert_eq!(
        borrow_vector_arithmetic_proof_exact_v2(&bad_scalar, 0).map(|_| ()),
        Err(VectorArithmeticCodecErrorV2::ScalarEncoding)
    );
    assert_eq!(
        borrow_vector_arithmetic_proof_exact_v2(&s3[..s3.len() - 1], 0).map(|_| ()),
        Err(VectorArithmeticCodecErrorV2::WireEncoding)
    );
}

#[test]
fn production_boundary_is_allocation_free_uninhabited_and_non_authorizing() {
    assert_eq!(readiness_gates_v2(), [0; 12]);
    assert!(
        BACKEND_ACCOUNTING_LANGUAGE_V2
            .windows(b"current-eager-backend-cannot-instantiate-any-shape".len())
            .any(|part| part == b"current-eager-backend-cannot-instantiate-any-shape")
    );
    assert!(
        BACKEND_ACCOUNTING_LANGUAGE_V2
            .windows(b"not-an-accepted-soundness-theorem".len())
            .any(|part| part == b"not-an-accepted-soundness-theorem")
    );
    let source = include_str!("vector_arithmetic_proof_codec_v2.rs");
    let production = source
        .split("#[cfg(test)]")
        .next()
        .expect("production source");
    for forbidden in [
        "Vec<",
        "vec![",
        "Box<",
        "to_vec(",
        "ArithmeticCircuitStatement",
        ".prove(",
        ".verify(",
    ] {
        assert!(
            !production.contains(forbidden),
            "forbidden production token: {forbidden}"
        );
    }
    for required in [
        "instantiated_streaming_backend: Infallible",
        "verified_equations: Infallible",
        "accepted_soundness_theorem: Infallible",
        "match instantiated_streaming_backend {}",
    ] {
        assert!(
            production.contains(required),
            "missing production seal: {required}"
        );
    }
}
