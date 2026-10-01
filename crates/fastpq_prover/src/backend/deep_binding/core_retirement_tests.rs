//! Independent structured canonical context vectors for the sole q77 protocol.
use super::*;

#[test]
fn structured_context_and_every_oracle_match_independent_norito_sha3_vectors() {
    let public = b"complete public context without a private witness";
    let descriptor = StatementContext {
        relation: "profile-binding:fixed-candidate:v1".to_owned(),
        layout: LAYOUT_ID.to_owned(),
        trace_rows: fixed_u32(TRACE_ROWS),
        lde_rows: fixed_u32(LDE_ROWS),
        columns: fixed_u32(COMMITTED_COLUMN_COUNT),
        constraints: fixed_u32(CONSTRAINTS),
        modulus: MODULUS,
        extension_nonresidue: 7,
        lde_root: LDE_ROOT,
        coset_offset: COSET_OFFSET,
        fri_arities: FRI_ARITIES.map(fixed_u32),
        fri_lengths: FRI_LENGTHS.map(fixed_u32),
        fri_degrees: FRI_DEGREES.map(fixed_u32),
        query_count: fixed_u32(QUERY_COUNT),
        query_candidates: fixed_u32(QUERY_CANDIDATES),
        statement: public.to_vec(),
    };
    // Independently encoded fixed-width values, array element prefixes, string
    // counts, struct field lengths, nominal schema hash and CRC64-XZ header.
    // Strings use compact value lengths under flag 0x02; the statement byte
    // vector keeps its fixed u64 sequence count. No runtime encoder derives
    // these expected bytes or the eight SHA3 oracle vectors below.
    assert_eq!(
        hex::encode(norito::encode_canonical(&descriptor).unwrap()),
        "4e52543000001ec15f6de0edd014c1b2850c0a244fad0069010000000000008d93a47c1df47da902232270726f66696c652d62696e64696e673a66697865642d63616e6469646174653a7631706f6661737470713a636f6d706163743a736d742d7075626c69632d636f6c756d6e733a76313a3334322d746f2d3330313a706572696f643531323a726f777336353533363a7075626c696333322d33352c35332d36332c3237362d3330313a6e6f646538333a6578656375746534303804000001000400008000042d010000049b0300000801000000ffffffff08070000000000000008b82ea64a8b52c4350846e98ea9f9690efd19041000000004100000000408000000040800000004040000001e0400008000040000080004008000000400100000040002000004800000001e040000020004002000000400020000044000000004080000000402000000044d0000000457000000393100000000000000636f6d706c657465207075626c696320636f6e7465787420776974686f757420612070726976617465207769746e657373"
    );
    let context = Context::with_identity("profile-binding:fixed-candidate:v1", public).unwrap();
    for (oracle, expected) in [
        (
            Oracle::Row,
            "d091f251c881ed910c022ef54fc9f85e4eee7b1b7f7cf87ec987f1bff0ac6a31",
        ),
        (
            Oracle::QuotientAndMask,
            "35c4e673bacfa80a68a8383b3e284c89a816fcbea8af2b60adf974c824a5211c",
        ),
        (
            Oracle::Fri(0),
            "5888201b92323961c3972b8612a93b0675898c378080bd28bcf554d8bdab65e8",
        ),
        (
            Oracle::Fri(1),
            "2c04cf48ea9519f66205be4afbeda18df81479e4babbc644c6ca59028a3a918c",
        ),
        (
            Oracle::Fri(2),
            "b9129e44663cc7d012c94a32ddd89c343ca3f3a9a265b73863b4c6b0555052ad",
        ),
        (
            Oracle::Fri(3),
            "b16d438ad490f3a712f81ae5efb61335f165b7fd453e0bba9b5f042d193d2f08",
        ),
        (
            Oracle::Fri(4),
            "c56ac6f9e933540f7d2439661797dd64ae2c72843e453eee3b41254675930a62",
        ),
        (
            Oracle::Terminal,
            "54460444c218566b2a943e0e2fc2a4d5352cff091bb484ef4a21d1e46f31396c",
        ),
    ] {
        let (_, _, _, size) = oracle.shape().unwrap();
        let expected = hex::decode(expected).unwrap();
        let bytes = vec![0; size];
        assert_eq!(
            context
                .hash_leaf(oracle, 0, &bytes)
                .unwrap()
                .into_bytes()
                .as_slice(),
            expected
        );
        let _ambient = norito::core::DecodeFlagsGuard::enter(0);
        assert_eq!(
            Context::with_identity("profile-binding:fixed-candidate:v1", public)
                .unwrap()
                .hash_leaf(oracle, 0, &bytes)
                .unwrap()
                .into_bytes()
                .as_slice(),
            expected
        );
    }
}
