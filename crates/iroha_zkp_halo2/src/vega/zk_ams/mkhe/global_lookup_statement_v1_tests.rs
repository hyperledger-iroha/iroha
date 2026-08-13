use super::*;
fn tiny_radix_v1(digits: &[u64], top: u64, base: u64) -> Option<u128> {
    if base < 2 || top > 1 || digits.iter().any(|digit| *digit >= base) {
        return None;
    }
    let mut value = u128::from(top);
    for digit in digits.iter().rev() {
        value = value.checked_mul(u128::from(base))? + u128::from(*digit);
    }
    Some(value)
}
fn mod_pow_v1(mut base: u64, mut exponent: u64, modulus: u64) -> u64 {
    let mut result = 1_u64;
    while exponent != 0 {
        if exponent & 1 == 1 {
            result = (u128::from(result) * u128::from(base) % u128::from(modulus)) as u64;
        }
        base = (u128::from(base) * u128::from(base) % u128::from(modulus)) as u64;
        exponent >>= 1;
    }
    result
}
fn log_derivative_v1(
    z: u64,
    values: &[u64],
    table: &[u64],
    multiplicities: &[u64],
    modulus: u64,
) -> Option<(u64, u64)> {
    if table.len() != multiplicities.len() || z >= modulus {
        return None;
    }
    let inverse = |value: u64| {
        (value < modulus && value != z)
            .then(|| mod_pow_v1((z + modulus - value) % modulus, modulus - 2, modulus))
    };
    let mut left = 0_u64;
    for value in values {
        left = ((u128::from(left) + u128::from(inverse(*value)?)) % u128::from(modulus)) as u64;
    }
    let mut right = 0_u64;
    for (value, multiplicity) in table.iter().zip(multiplicities) {
        right = ((u128::from(right) + u128::from(*multiplicity) * u128::from(inverse(*value)?))
            % u128::from(modulus)) as u64;
    }
    Some((left, right))
}
#[test]
fn exact_added_role_inventory_and_boundaries_are_bijective() {
    let mut counts = [0_usize; 12];
    let mut active = 0_usize;
    let mut inverse = 0_usize;
    for ordinal in 0..ADDED_PLANES_V1 {
        let coordinate = added_plane_coordinate_v1(ordinal).expect("added ordinal");
        assert_eq!(coordinate.ordinal, ordinal);
        counts[coordinate.role as usize - 1] += 1;
        active += usize::from(coordinate.active_lookup_slot.is_some());
        inverse += usize::from(coordinate.is_inverse_v1());
    }
    assert_eq!(
        counts,
        [
            5_848, 344, 6_192, 5_848, 1_032, 1_032, 1_032, 1_032, 6_080, 6_080, 6_080, 6_080
        ]
    );
    assert_eq!(
        (active, inverse),
        (ADDED_ACTIVE_PLANES_V1, ADDED_INVERSE_PLANES_V1)
    );
    for ordinal in [
        0,
        16,
        17,
        35,
        36,
        COMPARATOR_PLANES_V1 - 1,
        COMPARATOR_PLANES_V1,
        COMPARATOR_PLANES_V1 + SMALL_SOURCE_PLANES_V1 - 1,
        ADDED_PLANES_V1 - 1,
    ] {
        assert_eq!(added_plane_coordinate_v1(ordinal).unwrap().ordinal, ordinal);
    }
    assert_eq!(
        added_plane_coordinate_v1(ADDED_PLANES_V1),
        Err(GlobalLookupErrorV1::Shape)
    );
}
#[test]
fn active_lookup_and_virtual_padding_enumerate_exactly() {
    let mut existing = 0;
    let mut added = 0;
    let mut zero = 0;
    for slot in 0..PADDED_LOOKUP_PLANES_V1 {
        match lookup_plane_coordinate_v1(slot).expect("lookup slot") {
            LookupPlaneCoordinateV1::Existing { ordinal, .. } => {
                assert_eq!(ordinal, existing);
                existing += 1;
            }
            LookupPlaneCoordinateV1::Added(coordinate) => {
                assert_eq!(coordinate.active_lookup_slot, Some(slot));
                added += 1;
            }
            LookupPlaneCoordinateV1::VirtualZero { ordinal } => {
                assert_eq!(ordinal, zero);
                zero += 1;
            }
        }
    }
    assert_eq!((existing, added, zero), (11_696, 20_072, 1_000));
    assert_eq!(ACTIVE_LOOKUP_VALUES_V1, 31_768 * 16_384);
    assert_eq!(
        lookup_plane_coordinate_v1(PADDED_LOOKUP_PLANES_V1),
        Err(GlobalLookupErrorV1::Shape)
    );
}
#[test]
#[rustfmt::skip]
fn existing_d_and_slack_ranges_are_distinct() {
    for (slot, role) in [(0, ExistingLookupPlaneRoleV1::DDigit), (5_847, ExistingLookupPlaneRoleV1::DDigit), (5_848, ExistingLookupPlaneRoleV1::SlackDigit), (11_695, ExistingLookupPlaneRoleV1::SlackDigit)] {
        assert!(matches!(lookup_plane_coordinate_v1(slot), Ok(LookupPlaneCoordinateV1::Existing { ordinal, role: actual }) if ordinal == slot && actual == role));
    }
}
#[test]
#[rustfmt::skip]
fn active_role_ranges_are_contiguous_and_role_major() {
    for (start, end, role, owners) in [(17_544, 18_576, ActiveLookupRoleV1::SmallPositive, 1_032),
        (18_576, 19_608, ActiveLookupRoleV1::SmallNegativeMagnitude, 1_032),
        (19_608, 25_688, ActiveLookupRoleV1::QMaskDigit, 1_520),
        (25_688, 31_768, ActiveLookupRoleV1::QMaskComplementDigit, 1_520)] {
        for slot in start..end {
            let LookupPlaneCoordinateV1::Added(coordinate) = lookup_plane_coordinate_v1(slot).unwrap() else { panic!("expected added plane") };
            assert_eq!(coordinate.active_role, Some(role));
            assert_eq!(coordinate.owner, (slot - start) % owners);
            if owners == 1_520 { assert_eq!(coordinate.column, (slot - start) / owners); }
        }
    }
}
#[test]
#[rustfmt::skip]
fn equations_endpoints_ipas_and_exclusive_gate_schedule_are_exact() {
    for literal in [b"q_3[v]=sum_g(kappa^g*(bD_g[v]*(bD_g[v]-1)+delta*bS_g[v]*(bS_g[v]-1)+delta^2*bD_g[v]*bS_g[v]))".as_slice(), b"q_5[v]=sum_g(kappa^g*(sum_h=0..17(delta^h*beta_g,h[v]*(beta_g,h[v]-1))+delta^18*(m_g[v]-bD_g[v]*beta_g,16[v])+delta^19*(beta_g,17[v]-beta_g,16[v]+m_g[v])))", b"q_8[v]=sum_u(kappa^u*(x_u[v]+n_u[v])*n_u[v])", b"Q_3=MLE(q_3);Q_5=MLE(q_5);Q_8=MLE(q_8)", b"never-extend-these-products-off-the-Boolean-cube"] { assert!(SPECIAL_QUADRATIC_AGGREGATION_LANGUAGE_V1.windows(literal.len()).any(|window| window == literal)); }
    for literal in [b"q_s[v]=sum_o(kappa^o*sum_e(delta^e*R_{s,o,e}[v]))".as_slice(), b"Q_s(x)=MLE(q_s)(x)", b"for-linear-families-s-notin(3,5,8)-this-equals-the-direct-linear-residual-extension", b"F_s(x)=eq(tau,x)*Q_s(x)", b"initial-claim_s=0", b"unmasked-round-polynomial-per-variable-degree<=2-and-cubic-coefficient=0", b"existing-CompressedUnivariate-degree3-envelope-wire=(constant,quadratic,cubic)-canonical-le-96B", b"linear=claim-2*constant-quadratic-cubic", b"may-make-the-transmitted-cubic-coordinate-nonzero"] { assert!(COEFFICIENT_AGGREGATION_LANGUAGE_V1.windows(literal.len()).any(|window| window == literal)); }
    for literal in [b"A_s=Q_s(r_s)".as_slice(), b"A_s-opens-the-same-framed-q_s-commitment", b"or-the-same-verifier-derived-linear-aggregate", b"B_s=eq(tau,r_s)*A_s", b"Z_s=mask-terminal_s", b"gate0:B_s-eq(tau,r_s)*A_s=0", b"gate1:Cfinal_s-B_s-Z_s=0"] { assert!(COEFFICIENT_ENDPOINT_LANGUAGE_V1.windows(literal.len()).any(|window| window == literal)); }
    assert_eq!(POST_BATCH_RESIDUAL_VECTOR_LENGTH_V1, 1 << 14);
    assert!(POST_BATCH_RESIDUAL_REQUIREMENT_LANGUAGE_V1.windows(b"blinded-length-2^14-q_s-vector-commitment".len()).any(|window| window == b"blinded-length-2^14-q_s-vector-commitment"));
    assert!(POST_BATCH_RESIDUAL_REQUIREMENT_LANGUAGE_V1.windows(b"binding-every-q_s[v]-to-its-exact-frozen-Boolean-coordinate-formula".len()).any(|window| window == b"binding-every-q_s[v]-to-its-exact-frozen-Boolean-coordinate-formula"));
    assert!(POST_BATCH_RESIDUAL_REQUIREMENT_LANGUAGE_V1.windows(b"canonical-nonidentity-33B=99B".len()).any(|window| window == b"canonical-nonidentity-33B=99B"));
    assert!(POST_BATCH_RESIDUAL_REQUIREMENT_LANGUAGE_V1.windows(b"vector-proofs=3-required-but-codec-uninstantiated-and-wire-bytes-undefined".len()).any(|window| window == b"vector-proofs=3-required-but-codec-uninstantiated-and-wire-bytes-undefined"));
    assert_eq!(COEFFICIENT_EQUATION_ROLES_V1.len(), 14);
    for (ordinal, role) in COEFFICIENT_EQUATION_ROLES_V1.iter().enumerate() {
        assert_eq!(*role as usize, ordinal + 1);
        assert!(!role.formula_v1().is_empty());
        for endpoint in 0..3 {
            assert!(matches!(
                hidden_endpoint_role_v1(ordinal * 3 + endpoint).unwrap(),
                HiddenEndpointRoleV1::Equation { equation, .. } if equation == *role
            ));
        }
        assert_eq!(
            ipa_statement_role_v1(ordinal),
            Ok(IpaStatementRoleV1::Equation(*role))
        );
    }
    assert_eq!(
        hidden_endpoint_role_v1(41).unwrap(),
        HiddenEndpointRoleV1::Equation {
            equation: CoefficientEquationRoleV1::PackingTransposeSameOpening,
            endpoint: EquationEndpointV1::MaskTerminal,
        }
    );
    assert_eq!(
        hidden_endpoint_role_v1(42),
        Ok(HiddenEndpointRoleV1::GroupBinder(
            GroupBinderEndpointV1::Source
        ))
    );
    assert_eq!(
        hidden_endpoint_role_v1(51),
        Ok(HiddenEndpointRoleV1::GlobalLookup(
            LookupEndpointV1::Residual
        ))
    );
    assert_eq!(hidden_endpoint_role_v1(52), Err(GlobalLookupErrorV1::Shape));
    assert_eq!(
        ipa_statement_role_v1(14),
        Ok(IpaStatementRoleV1::GroupBinder)
    );
    assert_eq!(
        ipa_statement_role_v1(15),
        Ok(IpaStatementRoleV1::GlobalLookup)
    );
    assert_eq!(ipa_statement_role_v1(16), Err(GlobalLookupErrorV1::Shape));
    for statement in 0..ENDPOINT_STATEMENTS_V1 {
        assert_eq!(
            endpoint_gate_coordinate_v1(2 * statement)
                .unwrap()
                .statement_ordinal,
            statement
        );
        assert_eq!(
            endpoint_gate_coordinate_v1(2 * statement + 1)
                .unwrap()
                .statement_ordinal,
            statement
        );
    }
    assert_eq!(ENDPOINT_GATES_V1, 32);
    assert_eq!(
        endpoint_gate_coordinate_v1(31).unwrap().statement_ordinal,
        15
    );
    assert_eq!(
        endpoint_gate_coordinate_v1(32),
        Err(GlobalLookupErrorV1::Shape)
    );
}
#[test]
#[rustfmt::skip]
fn cubic_extension_tiny_equations_and_log_derivative_oracles_hold() {
    for (ordinal, role, equation, local_round) in [(0, CubicMessageRoleV1::Equation, Some(CoefficientEquationRoleV1::DRadixReconstruction), 0), (13, CubicMessageRoleV1::Equation, Some(CoefficientEquationRoleV1::DRadixReconstruction), 13), (14, CubicMessageRoleV1::Equation, Some(CoefficientEquationRoleV1::SlackRadixReconstruction), 0), (181, CubicMessageRoleV1::Equation, Some(CoefficientEquationRoleV1::SourceCoefficientSameOpening), 13), (182, CubicMessageRoleV1::Equation, Some(CoefficientEquationRoleV1::PackingTransposeSameOpening), 0), (195, CubicMessageRoleV1::Equation, Some(CoefficientEquationRoleV1::PackingTransposeSameOpening), 13), (196, CubicMessageRoleV1::GroupBinder, None, 0), (204, CubicMessageRoleV1::GroupBinder, None, 8), (205, CubicMessageRoleV1::GlobalLookup, None, 0), (233, CubicMessageRoleV1::GlobalLookup, None, 28)] {
        let coordinate = cubic_message_coordinate_v1(ordinal).unwrap();
        assert_eq!((coordinate.role, coordinate.equation, coordinate.local_round), (role, equation, local_round));
        assert_eq!(coordinate.extends_prior_schedule, ordinal == 233);
    }
    assert_eq!(
        cubic_message_coordinate_v1(234),
        Err(GlobalLookupErrorV1::Shape)
    );
    let d = tiny_radix_v1(&[4, 2], 0, 5).unwrap();
    let slack = tiny_radix_v1(&[0, 2], 1, 5).unwrap();
    assert_eq!((d, slack, d + slack), (14, 35, 49));
    assert_eq!(tiny_radix_v1(&[5], 0, 5), None);
    let candidates = [0, 1, 1, 3];
    let table = [0, 1, 2, 3];
    let multiplicities = [1, 2, 0, 1];
    let (left, right) = log_derivative_v1(7, &candidates, &table, &multiplicities, 97).unwrap();
    assert_eq!(left, right);
    assert_ne!(
        left,
        log_derivative_v1(7, &candidates, &table, &[1, 1, 1, 1], 97)
            .unwrap()
            .1
    );
    assert_eq!(
        log_derivative_v1(1, &candidates, &table, &multiplicities, 97),
        None
    );
}
#[test]
#[rustfmt::skip]
fn centering_boundary_recurrence_has_exact_sign_and_eighteen_borrows() {
    let (base, p_t, k) = (5_i128, 41_i128, 21_i128);
    for d in 0..p_t { let digits=[d%base,(d/base)%base]; let k_digits=[k%base,(k/base)%base]; let b_d=d/(base*base); let mut accepted=0; for delta_0 in 0..base { for delta_1 in 0..base { for borrow_0 in 0..=1 { for borrow_1 in 0..=1 { for mixed_top in 0..=1 { for c in 0..=1 { let ok=digits[0]-k_digits[0]==delta_0-base*borrow_0 && digits[1]-k_digits[1]-borrow_0==delta_1-base*borrow_1 && mixed_top==b_d*borrow_1 && c==borrow_1-mixed_top; if ok { accepted+=1; assert_eq!(c,i128::from(d<k)); assert_eq!(d-(1-c)*p_t,if d<k {d}else{d-p_t}); } } } } } } } assert_eq!(accepted,1); }
    let forged_old_witness = |d: i128, c: i128, delta: i128| d + c * p_t - k == delta;
    assert!(forged_old_witness(k,1,p_t));
    assert_ne!(1_i128, i128::from(k<k));
    let subtraction = CoefficientEquationRoleV1::CenteringComparatorSubtraction.formula_v1();
    let booleanity = CoefficientEquationRoleV1::ComparatorBorrowBooleanity.formula_v1();
    let formulas = [subtraction, booleanity, CoefficientEquationRoleV1::CenteredLiftSelector.formula_v1()].concat();
    assert!(formulas.windows(b"for-h=0..17".len()).any(|w| w == b"for-h=0..17"));
    assert!(formulas.windows(b"for-h=1..16".len()).any(|w| w == b"for-h=1..16"));
    assert!(formulas.windows(b"K_17=0".len()).any(|w| w == b"K_17=0"));
    assert!(!subtraction.windows(b"m_g=".len()).any(|w| w == b"m_g="));
    assert!(!subtraction.windows(b"beta_g,17=".len()).any(|w| w == b"beta_g,17="));
    assert!(booleanity.windows(b"m_g=bD_g*beta_g,16".len()).any(|w| w == b"m_g=bD_g*beta_g,16"));
    assert!(booleanity.windows(b"beta_g,17=beta_g,16-m_g".len()).any(|w| w == b"beta_g,17=beta_g,16-m_g"));
    assert!(!formulas.windows(b"0<=Delta<pT".len()).any(|w| w == b"0<=Delta<pT"));
    assert!(!formulas.windows(b"borrow_{h+1}".len()).any(|w| w == b"borrow_{h+1}"));
    assert!(!formulas.windows(b"b18".len()).any(|w| w == b"b18"));
}
#[test]
#[rustfmt::skip]
fn independent_lookup_language_and_telescoping_mask_oracle_hold() {
    let (p,z,rho,alpha,lambda,mu)=(97_u64,7_u64,[2_u64,3,5],11_u64,13_u64,17_u64);
    let multiplicities=[1_u64,2,2,1];
    let evaluate=|candidates:[u64;8]| { let mut total=0_u64; for (index,a) in candidates.into_iter().enumerate() { let coordinate=index&1; let plane=index>>1; let active=u64::from(plane<3); let inverse=active*mod_pow_v1((z+p-a)%p,p-2,p)%p; let bits=[coordinate,plane&1,(plane>>1)&1]; let equality=bits.into_iter().zip(rho).fold(1_u64,|value,(bit,challenge)| (u128::from(value)*u128::from(if bit==1 {challenge}else{(1+p-challenge)%p})%u128::from(p)) as u64); let e0=u64::from(coordinate==0); let table_inverse=mod_pow_v1((z+p-plane as u64)%p,p-2,p); let local=((z+p-a)%p*inverse+p-active)%p; let log=(inverse+p-(e0*multiplicities[plane]%p)*table_inverse%p)%p; let count=(e0*multiplicities[plane]+p-active)%p; total=(total+alpha*equality%p*local+lambda*log+mu*count)%p; } total };
    assert_eq!(evaluate([0,1,1,2,2,3,0,0]),0);
    assert_eq!(evaluate([4,1,1,2,2,3,0,0]),81);
    let mut carry=0_u64; for (challenge,[a,b,c]) in [(9_u64,[2_u64,4,6]),(10,[3,5,7])] { let d=(carry+p-a-b-c)%p*49%p; let mask=|x:u64| (((a*x%p*x%p*x+b*x%p*x+c*x+d)%p)); assert_eq!((mask(0)+mask(1))%p,carry); carry=mask(challenge); }
    assert_ne!(carry,0);
    for schema in [LOOKUP_INDEX_LANGUAGE_V1,LOOKUP_RELATION_LANGUAGE_V1,LOOKUP_MASK_LANGUAGE_V1,LOOKUP_ENDPOINT_LANGUAGE_V1,LOOKUP_SOUNDNESS_LANGUAGE_V1] { assert!(!schema.is_empty()); }
}
#[test]
#[rustfmt::skip]
fn conditional_accounting_soundness_and_fail_closed_gates_are_frozen() {
    assert_eq!(conditional_accounting_v1(), None);
    assert_eq!(KNOWN_WIRE_LOWER_BOUND_BEFORE_VECTOR_ARITHMETIC_PROOFS_V1, 33_230_654);
    assert_eq!((CONDITIONAL_TOTAL_BYTES_V1, CONDITIONAL_MARGIN_BYTES_V1), (None, None));
    assert_eq!(NEW_MASK_COMMITMENT_AND_IPA_BYTES_V1, 1_150);
    assert_eq!(MASK_IPA_CORRECTION_BYTES_V1, 425);
    assert_eq!(GLOBAL_LOOKUP_DELTA_BYTES_V1, 6_624);
    assert_eq!(COEFFICIENT_CHALLENGE_WIRE_BYTES_V1, 0);
    assert_eq!(POST_BATCH_RESIDUAL_STATEMENTS_V1, [3, 5, 8]);
    assert_eq!(POST_BATCH_RESIDUAL_COMMITMENT_BYTES_V1, 99);
    assert_eq!((REQUIRED_POST_BATCH_RESIDUAL_COMMITMENTS_V1, REQUIRED_VECTOR_ARITHMETIC_PROOFS_V1), (3, 3));
    assert!(POST_BATCH_RESIDUAL_COMMITMENT_FRAMES_INSTANTIATED_V1 && !VECTOR_ARITHMETIC_PROOFS_INSTANTIATED_V1);
    assert_eq!(
        LOOKUP_SOUNDNESS_NUMERATOR_V1,
        ACTIVE_LOOKUP_VALUES_V1 + 32_768 - 2
    );
    assert_eq!(LOOKUP_SOUNDNESS_BITS_X100_FLOOR_V1, 22_704);
    let modulus_high = u64::from_be_bytes(VEGA_T256_SCALAR_MODULUS_BE_V1[..8].try_into().unwrap());
    assert!((LOOKUP_SOUNDNESS_NUMERATOR_V1 << 35) < modulus_high);
    assert!(SOUNDNESS_FORMULA_V1.ends_with(b"520486912+32768-2"));
    for gate in [
        LOOKUP_PROOF_VERIFIED_V1,
        ZERO_KNOWLEDGE_ACCEPTED_V1,
        SOURCE_SAME_OPENING_VERIFIED_V1,
        PACKING_SAME_OPENING_VERIFIED_V1,
        CROSS_FIELD_BINDING_VERIFIED_V1,
        STREAMING_OWNERS_WIRED_V1,
        COMPLETE_ACCOUNTING_QUALIFIED_V1,
        OPERATIONAL_RECEIPT_ACCEPTED_V1,
        AUTHORITY_MINTED_V1,
        RSS_QUALIFIED_V1,
        RELEASE_READY_V1,
    ] {
        assert!(!gate);
    }
}
#[test]
#[rustfmt::skip]
fn opaque_owner_and_source_guards_are_static() {
    let production = include_str!("global_lookup_statement_v1.rs");
    let challenges = include_str!("global_lookup_statement_v1/challenge_v1.rs");
    let parent = include_str!("../mkhe.rs");
    assert!(production.lines().count() <= 900);
    assert!(include_str!("global_lookup_statement_v1_tests.rs").lines().count() <= 450);
    assert_eq!(parent.matches("mod global_lookup_statement_v1;").count(), 1);
    assert!(!production.contains("pub struct"));
    assert!(!production.contains("pub trait"));
    assert!(!production.contains("Vec<"));
    assert!(!production.contains("&[Point]"));
    assert!(!production.contains("impl Clone for BoundOwnerSealsV1"));
    assert!(production.contains("trait OpaqueStreamingOwnerV1<Stage>: Sized"));
    assert!(production.contains("hash.update(&challenge_manifest_digest_v1())"));
    assert!(challenges.contains("FIRST_SUMCHECK_ORDINAL_V1 == DELTA_ORDINAL_V1 + 1"));
    for seal in ["enum SourcePackingOwnerSealV1", "enum LookupOwnerSealV1", "enum ProofOwnerSealV1"] {
        let body = production.split(seal).nth(1).unwrap().split("}\n").next().unwrap();
        assert!(body.contains("Production"));
        assert!(body.contains("Infallible"));
    }
}
