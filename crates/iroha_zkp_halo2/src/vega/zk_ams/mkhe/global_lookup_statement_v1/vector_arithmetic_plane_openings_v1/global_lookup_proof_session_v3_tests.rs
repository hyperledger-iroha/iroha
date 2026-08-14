use super::*;

use core::sync::atomic::Ordering;

#[derive(Clone, Copy, PartialEq, Eq)]
enum RustDeclarationTokenV3<'source> {
    Word(&'source str),
    Punct(u8),
    Literal(&'source str),
}

fn rust_declaration_tokens_v3(source: &str) -> Option<Vec<RustDeclarationTokenV3<'_>>> {
    use RustDeclarationTokenV3::{Literal, Punct, Word};

    fn raw_string_prefix(bytes: &[u8], offset: usize) -> Option<(usize, usize)> {
        let mut cursor = offset;
        if matches!(bytes.get(cursor), Some(b'b' | b'c')) {
            cursor += 1;
        }
        if bytes.get(cursor) != Some(&b'r') {
            return None;
        }
        cursor += 1;
        let hashes_start = cursor;
        while bytes.get(cursor) == Some(&b'#') {
            cursor += 1;
        }
        (bytes.get(cursor) == Some(&b'"')).then_some((cursor, cursor - hashes_start))
    }

    fn quoted_char_literal_end(source: &str, offset: usize) -> Option<usize> {
        let bytes = source.as_bytes();
        let (quote, byte_char) =
            if bytes.get(offset) == Some(&b'b') && bytes.get(offset + 1) == Some(&b'\'') {
                (offset + 1, true)
            } else if bytes.get(offset) == Some(&b'\'') {
                (offset, false)
            } else {
                return None;
            };
        let mut cursor = quote + 1;
        if bytes.get(cursor) == Some(&b'\\') {
            cursor += 1;
            match *bytes.get(cursor)? {
                b'\'' | b'"' | b'\\' | b'n' | b'r' | b't' | b'0' => cursor += 1,
                b'x' => {
                    let high = *bytes.get(cursor + 1)?;
                    let low = *bytes.get(cursor + 2)?;
                    if !high.is_ascii_hexdigit()
                        || !low.is_ascii_hexdigit()
                        || (!byte_char && high > b'7')
                    {
                        return None;
                    }
                    cursor += 3;
                }
                b'u' if !byte_char && bytes.get(cursor + 1) == Some(&b'{') => {
                    cursor += 2;
                    let mut digits = 0_u8;
                    let mut scalar = 0_u32;
                    while let Some(byte) = bytes.get(cursor) {
                        if *byte == b'}' {
                            break;
                        }
                        if *byte != b'_' {
                            let digit = (*byte as char).to_digit(16)?;
                            digits = digits.checked_add(1)?;
                            if digits > 6 {
                                return None;
                            }
                            scalar = scalar.checked_mul(16)?.checked_add(digit)?;
                        }
                        cursor += 1;
                    }
                    if digits == 0
                        || bytes.get(cursor) != Some(&b'}')
                        || char::from_u32(scalar).is_none()
                    {
                        return None;
                    }
                    cursor += 1;
                }
                _ => return None,
            }
        } else if byte_char {
            let byte = *bytes.get(cursor)?;
            if !byte.is_ascii() || matches!(byte, b'\'' | b'\\' | b'\n' | b'\r' | b'\t') {
                return None;
            }
            cursor += 1;
        } else {
            let character = source.get(cursor..)?.chars().next()?;
            if matches!(character, '\'' | '\\' | '\n' | '\r' | '\t') {
                return None;
            }
            cursor += character.len_utf8();
        }
        (bytes.get(cursor) == Some(&b'\'')).then_some(cursor + 1)
    }

    let bytes = source.as_bytes();
    let mut tokens = Vec::new();
    let mut offset = 0;
    while offset < bytes.len() {
        if bytes[offset].is_ascii_whitespace() {
            offset += 1;
            continue;
        }
        if bytes[offset..].starts_with(b"//") {
            offset += 2;
            while offset < bytes.len() && bytes[offset] != b'\n' {
                offset += 1;
            }
            continue;
        }
        if bytes[offset..].starts_with(b"/*") {
            offset += 2;
            let mut depth = 1_u32;
            while depth != 0 {
                if bytes.get(offset..offset + 2) == Some(b"/*") {
                    depth += 1;
                    offset += 2;
                } else if bytes.get(offset..offset + 2) == Some(b"*/") {
                    depth -= 1;
                    offset += 2;
                } else {
                    offset = offset.checked_add(1).filter(|next| *next <= bytes.len())?;
                }
            }
            continue;
        }
        if let Some((quote, hashes)) = raw_string_prefix(bytes, offset) {
            let start = offset;
            offset = quote + 1;
            loop {
                let relative_quote = bytes[offset..].iter().position(|byte| *byte == b'"')?;
                offset += relative_quote + 1;
                if bytes.get(offset..offset + hashes) == Some(&bytes[quote - hashes..quote]) {
                    offset += hashes;
                    break;
                }
            }
            tokens.push(Literal(&source[start..offset]));
            continue;
        }
        if bytes[offset] == b'"' {
            let start = offset;
            offset += 1;
            loop {
                match *bytes.get(offset)? {
                    b'\\' => offset = offset.checked_add(2).filter(|next| *next <= bytes.len())?,
                    b'"' => {
                        offset += 1;
                        break;
                    }
                    _ => offset += 1,
                }
            }
            tokens.push(Literal(&source[start..offset]));
            continue;
        }
        if let Some(end) = quoted_char_literal_end(source, offset) {
            tokens.push(Literal(&source[offset..end]));
            offset = end;
            continue;
        }
        if bytes[offset].is_ascii_alphabetic() || bytes[offset] == b'_' {
            let start = offset;
            offset += 1;
            while bytes
                .get(offset)
                .is_some_and(|byte| byte.is_ascii_alphanumeric() || *byte == b'_')
            {
                offset += 1;
            }
            tokens.push(Word(&source[start..offset]));
            continue;
        }
        tokens.push(Punct(bytes[offset]));
        offset += 1;
    }
    Some(tokens)
}

fn has_exact_private_module_declaration_v3(parent: &str) -> bool {
    use RustDeclarationTokenV3::{Punct, Word};

    const MODULE_NAME: &str = "global_lookup_proof_session_v3";
    let Some(tokens) = rust_declaration_tokens_v3(parent) else {
        return false;
    };

    let mut declaration_start = 0;
    let mut delimiter_depth = [0_u32; 3];
    let mut exact_private_declaration = None;
    for index in 0..tokens.len() {
        let top_level = delimiter_depth == [0; 3];
        if top_level
            && tokens.get(index) == Some(&Word("mod"))
            && tokens.get(index + 1) == Some(&Word(MODULE_NAME))
            && tokens.get(index + 2) == Some(&Punct(b';'))
        {
            if exact_private_declaration.is_some() {
                return false;
            }
            let prefix = &tokens[declaration_start..index];
            let mut cursor = 0;
            let mut only_outer_attributes = true;
            while cursor < prefix.len() {
                if prefix.get(cursor) != Some(&Punct(b'#'))
                    || prefix.get(cursor + 1) != Some(&Punct(b'['))
                {
                    only_outer_attributes = false;
                    break;
                }
                cursor += 2;
                let mut attribute_depth = 1_u32;
                while attribute_depth != 0 {
                    match prefix.get(cursor) {
                        Some(Punct(b'[')) => attribute_depth += 1,
                        Some(Punct(b']')) => attribute_depth -= 1,
                        Some(_) => {}
                        None => {
                            only_outer_attributes = false;
                            break;
                        }
                    }
                    cursor += 1;
                }
            }
            exact_private_declaration = Some(only_outer_attributes);
        }

        match tokens[index] {
            Punct(b'(') => delimiter_depth[0] += 1,
            Punct(b')') => {
                let Some(depth) = delimiter_depth[0].checked_sub(1) else {
                    return false;
                };
                delimiter_depth[0] = depth;
            }
            Punct(b'[') => delimiter_depth[1] += 1,
            Punct(b']') => {
                let Some(depth) = delimiter_depth[1].checked_sub(1) else {
                    return false;
                };
                delimiter_depth[1] = depth;
            }
            Punct(b'{') => delimiter_depth[2] += 1,
            Punct(b'}') => {
                let Some(depth) = delimiter_depth[2].checked_sub(1) else {
                    return false;
                };
                delimiter_depth[2] = depth;
            }
            Punct(b';') if top_level => declaration_start = index + 1,
            _ => {}
        }
        if !top_level && delimiter_depth == [0; 3] && matches!(tokens[index], Punct(b'}')) {
            declaration_start = index + 1;
        }
    }
    (delimiter_depth == [0; 3] && exact_private_declaration == Some(true))
}

fn has_exact_private_session_declaration_v3(source: &str) -> bool {
    use RustDeclarationTokenV3::{Literal, Punct, Word};

    const ATTRIBUTE: &[RustDeclarationTokenV3<'static>] = &[
        Punct(b'#'),
        Punct(b'['),
        Word("must_use"),
        Punct(b'='),
        Literal("\"dropping the session destroys its sole live structural owner\""),
        Punct(b']'),
    ];
    const FIELDS: &[RustDeclarationTokenV3<'static>] = &[
        Word("live"),
        Punct(b':'),
        Word("Option"),
        Punct(b'<'),
        Word("GlobalLookupProofSessionLiveV3"),
        Punct(b'<'),
        Word("R"),
        Punct(b'>'),
        Punct(b'>'),
        Punct(b','),
        Word("poisoned"),
        Punct(b':'),
        Word("bool"),
        Punct(b','),
        Word("state"),
        Punct(b':'),
        Word("PhantomData"),
        Punct(b'<'),
        Word("State"),
        Punct(b'>'),
        Punct(b','),
    ];

    let Some(tokens) = rust_declaration_tokens_v3(source) else {
        return false;
    };

    let mut declaration_start = 0;
    let mut delimiter_depth = [0_u32; 3];
    let mut exact_private_declaration = None;
    for index in 0..tokens.len() {
        let top_level = delimiter_depth == [0; 3];
        if top_level
            && tokens.get(index) == Some(&Word("struct"))
            && tokens.get(index + 1) == Some(&Word("GlobalLookupProofSessionV3"))
            && tokens.get(index + 2) == Some(&Punct(b'<'))
            && tokens.get(index + 3) == Some(&Word("R"))
            && tokens.get(index + 4) == Some(&Punct(b','))
            && tokens.get(index + 5) == Some(&Word("State"))
            && tokens.get(index + 6) == Some(&Punct(b'>'))
            && tokens.get(index + 7) == Some(&Punct(b'{'))
        {
            if exact_private_declaration.is_some() {
                return false;
            }
            let mut body_depth = 0_u32;
            let mut declaration_end = None;
            for (cursor, token) in tokens.iter().enumerate().skip(index + 7) {
                match token {
                    Punct(b'{') => body_depth += 1,
                    Punct(b'}') => {
                        let Some(depth) = body_depth.checked_sub(1) else {
                            return false;
                        };
                        body_depth = depth;
                        if body_depth == 0 {
                            declaration_end = Some(cursor);
                            break;
                        }
                    }
                    _ => {}
                }
            }
            let Some(declaration_end) = declaration_end else {
                return false;
            };
            let region = &tokens[declaration_start..=declaration_end];
            let forbidden = region.iter().any(|token| {
                matches!(
                    token,
                    Word("derive" | "Clone" | "Copy" | "Debug" | "Default" | "Deref" | "pub")
                )
            });
            exact_private_declaration = Some(
                !forbidden
                    && &tokens[declaration_start..index] == ATTRIBUTE
                    && &tokens[index + 8..declaration_end] == FIELDS,
            );
        }

        match tokens[index] {
            Punct(b'(') => delimiter_depth[0] += 1,
            Punct(b')') => {
                let Some(depth) = delimiter_depth[0].checked_sub(1) else {
                    return false;
                };
                delimiter_depth[0] = depth;
            }
            Punct(b'[') => delimiter_depth[1] += 1,
            Punct(b']') => {
                let Some(depth) = delimiter_depth[1].checked_sub(1) else {
                    return false;
                };
                delimiter_depth[1] = depth;
            }
            Punct(b'{') => delimiter_depth[2] += 1,
            Punct(b'}') => {
                let Some(depth) = delimiter_depth[2].checked_sub(1) else {
                    return false;
                };
                delimiter_depth[2] = depth;
            }
            Punct(b';') if top_level => declaration_start = index + 1,
            _ => {}
        }
        if !top_level && delimiter_depth == [0; 3] && matches!(tokens[index], Punct(b'}')) {
            declaration_start = index + 1;
        }
    }
    delimiter_depth == [0; 3] && exact_private_declaration == Some(true)
}

#[test]
fn physical_manifest_is_exact_and_contiguous() {
    use PhysicalPhaseV3::PostDeltaResidual as Residual;
    use PhysicalPhaseV3::{ChallengeIndependent as PreZ, JointPostZ as PostZ};
    use PhysicalPurposeV3::*;

    #[rustfmt::skip]
    let expected = [
        (PreZ, Source, 0, 344),
        (PreZ, ExistingDifferenceLow, 344, 5_848),
        (PreZ, ExistingSumLow, 6_192, 5_848),
        (PreZ, ComparatorDifferenceTop, 12_040, 344),
        (PreZ, ComparatorSumTop, 12_384, 344),
        (PreZ, ComparatorDifferenceDigit, 12_728, 5_848),
        (PreZ, ComparatorBorrow, 18_576, 6_192),
        (PreZ, ComparatorMixedTop, 24_768, 344),
        (PreZ, SmallSigned, 25_112, 1_032),
        (PreZ, SmallNegativeMagnitude, 26_144, 1_032),
        (PreZ, QMaskDigit, 27_176, 6_080),
        (PreZ, QMaskComplementDigit, 33_256, 6_080),
        (PreZ, Multiplicity, 39_336, 1),
        (PreZ, SumcheckMask, 39_337, 1),
        (PostZ, SharedDifferenceInverse, 39_338, 5_848),
        (PostZ, SharedSumInverse, 45_186, 5_848),
        (PostZ, ComparatorDifferenceInverse, 51_034, 5_848),
        (PostZ, SmallSignedInverse, 56_882, 1_032),
        (PostZ, SmallNegativeInverse, 57_914, 1_032),
        (PostZ, QMaskDigitInverse, 58_946, 6_080),
        (PostZ, QMaskComplementInverse, 65_026, 6_080),
        (Residual, ResidualQ3, 71_106, 1),
        (Residual, ResidualQ5, 71_107, 1),
        (Residual, ResidualQ8, 71_108, 1),
    ];

    let mut cursor = 0;
    let mut phase_counts = [0_u32; 3];
    for (range, (phase, purpose, first, count)) in PHYSICAL_ROLE_RANGES_V3.iter().zip(expected) {
        assert_eq!(
            (range.phase, range.purpose, range.first, range.count),
            (phase, purpose, first, count)
        );
        assert_eq!(range.first, cursor);
        cursor += range.count;
        phase_counts[range.phase as usize - 1] += range.count;
    }
    assert_eq!(cursor, PHYSICAL_INVENTORY_V3);
    assert_eq!(phase_counts, [PRE_Z_COMMITMENTS_V3, GLOBAL_INVERSES_V3, 3]);
    assert_eq!(PHYSICAL_INVENTORY_V3, 71_109);
    assert_eq!(SOURCE_COMMITMENTS_V3, 344);
    assert_eq!(SUFFIX_AFTER_SOURCE_V3, 70_765);
    assert_eq!(POST_DELTA_RESIDUALS_V3, 3);
}

#[test]
fn alias_manifest_is_exact_and_never_aliases_source() {
    use AliasPurposeV3::*;
    let expected = [
        (BooleanD, 0, 12_040, 344),
        (BooleanS, 344, 12_384, 344),
        (ComparatorBorrow, 688, 18_576, 6_192),
        (MixedTop, 6_880, 24_768, 344),
        (SmallSigned, 7_224, 25_112, 1_032),
        (SmallNegativeMagnitude, 8_256, 26_144, 1_032),
        (ResidualQ3, 9_288, 71_106, 1),
        (ResidualQ5, 9_289, 71_107, 1),
        (ResidualQ8, 9_290, 71_108, 1),
    ];

    let mut logical_cursor = 0;
    for (range, (purpose, logical_first, physical_first, count)) in
        ALIAS_RANGES_V3.iter().zip(expected)
    {
        assert_eq!(
            (
                range.purpose,
                range.logical_first,
                range.physical_first,
                range.count
            ),
            (purpose, logical_first, physical_first, count)
        );
        assert_eq!(range.logical_first, logical_cursor);
        assert!(range.physical_first >= 12_040);
        logical_cursor += range.count;
    }
    assert_eq!(logical_cursor, VECTOR_ALIAS_COUNT_V3);
    for logical_ordinal in 0..VECTOR_ALIAS_COUNT_V3 {
        let coordinate = alias_coordinate_v3(logical_ordinal).unwrap();
        assert_eq!(coordinate.logical_ordinal, logical_ordinal);
        assert!(coordinate.physical_ordinal >= 12_040);
        assert!(coordinate.physical_ordinal >= SOURCE_COMMITMENTS_V3);
    }
    assert_eq!(
        alias_coordinate_v3(VECTOR_ALIAS_COUNT_V3),
        Err(ProofSessionStructuralErrorV3::Alias)
    );
}

#[test]
fn storage_read_replay_and_completion_ledgers_are_exact() {
    assert_eq!(SUFFIX_SEMANTIC_RECORD_BYTES_V3, 65);
    assert_eq!(AUTHENTICATION_TAG_BYTES_V3, 16);
    assert_eq!(SUFFIX_PLAINTEXT_BYTES_V3, 4_599_725);
    assert_eq!(SUFFIX_TAG_BYTES_V3, 1_132_240);
    assert_eq!(SUFFIX_FILE_BYTES_V3, 5_731_965);
    assert_eq!(SUFFIX_WRITE_AND_SEAL_BYTES_V3, 11_463_930);
    assert_eq!(COMPOSITE_BLINDING_BYTES_V3, 2_275_488);
    assert_eq!(COMPOSITE_POINT_BYTES_V3, 2_346_597);
    assert_eq!(COMPOSITE_SEMANTIC_BYTES_V3, 4_622_085);
    assert_eq!(COMPOSITE_TAG_BYTES_V3, 1_137_744);
    assert_eq!(COMPOSITE_FILE_BYTES_V3, 5_759_829);
    assert_eq!(PLANE_SLOTS_V3, 306_603);
    assert_eq!(PLANE_FILE_BYTES_V3, 5_028_289_200);
    assert_eq!(
        (
            AUXILIARY_READ_A_BYTES_V3,
            AUXILIARY_READ_B_BYTES_V3,
            AUXILIARY_READ_C_BYTES_V3
        ),
        (33_849_600, 33_849_600, 752_571)
    );
    assert_eq!(AUXILIARY_READ_BYTES_V3, 68_451_771);
    assert_eq!(
        (
            TENSOR_TERM_REPLAY_BYTES_V3,
            TERMINAL_AGGREGATE_REPLAY_BYTES_V3,
            COEFFICIENT_IPA_REPLAY_BYTES_V3,
            ENDPOINT_OPENING_REPLAY_BYTES_V3
        ),
        (68_262_835_200, 4_875_916_800, 152_372_400, 1_623_600)
    );
    assert_eq!(REPLAY_READ_BYTES_V3, 73_292_748_000);
    assert_eq!(PLANE_WRITE_AND_SEAL_BYTES_V3, 10_056_578_400);
    assert_eq!(STRUCTURAL_IO_BYTES_V3, 83_349_326_400);
    assert_eq!(REPLAY_COMPLETION_MASK_V3, 0x3ffff);
    assert_eq!(REPLAY_PURPOSES_V3.len(), 18);
    assert!(
        core::str::from_utf8(HEAP_LANGUAGE_V3)
            .unwrap()
            .contains("not-an-RSS-claim")
    );
    assert!(
        core::str::from_utf8(OWNER_REHOME_LANGUAGE_V3)
            .unwrap()
            .contains("as-siblings;no-recursive-radix-nesting")
    );
}

#[test]
fn entropy_axes_and_retry_bound_are_lexicographic() {
    use EntropyPhaseV3::{PreZ, SoleZ};
    use EntropyPurposeV3::{ChallengeRejection as Challenge, CommitmentBlinding as Blinding};
    let coordinates = [
        EntropyCoordinateV3::new_v3(PreZ, Blinding, 5, 0).unwrap(),
        EntropyCoordinateV3::new_v3(PreZ, Blinding, 5, 1).unwrap(),
        EntropyCoordinateV3::new_v3(PreZ, Blinding, 6, 0).unwrap(),
        EntropyCoordinateV3::new_v3(PreZ, Challenge, 0, 0).unwrap(),
        EntropyCoordinateV3::new_v3(SoleZ, Blinding, 0, 0).unwrap(),
    ];
    assert!(coordinates.windows(2).all(|pair| pair[0] < pair[1]));
    assert!(EntropyCoordinateV3::new_v3(PreZ, Blinding, u32::MAX, 127).is_ok());
    assert_eq!(
        EntropyCoordinateV3::new_v3(PreZ, Blinding, 0, 128),
        Err(ProofSessionStructuralErrorV3::Entropy)
    );
}

#[test]
fn permits_are_removed_before_failures_and_complete_once() {
    use ProofReplayPurposeV3::*;
    #[rustfmt::skip]
    let expected = [
        TensorTermRound0, TensorTermRound1, TensorTermRound2, TensorTermRound3,
        TensorTermRound4, TensorTermRound5, TensorTermRound6, TensorTermRound7,
        TensorTermRound8, TensorTermRound9, TensorTermRound10, TensorTermRound11,
        TensorTermRound12, TensorTermRound13, TerminalAggregate,
        CoefficientIpa3, CoefficientIpa5, CoefficientIpa8,
    ];
    assert_eq!(REPLAY_PURPOSES_V3, expected);

    let mut validation = ProofReplayPermitsV3::new_v3();
    assert!(matches!(
        validation.take_for_test_v3(
            ProofReplayPurposeV3::TensorTermRound0,
            TestReplayOutcomeV3::ValidationError
        ),
        Err(ProofSessionStructuralErrorV3::Validation)
    ));
    assert!(validation.slots[0].is_none());
    assert!(matches!(
        validation.remove_v3(ProofReplayPurposeV3::TensorTermRound0),
        Err(ProofSessionStructuralErrorV3::PermitConsumed)
    ));

    let mut io = ProofReplayPermitsV3::new_v3();
    assert!(matches!(
        io.take_for_test_v3(
            ProofReplayPurposeV3::CoefficientIpa8,
            TestReplayOutcomeV3::IoError
        ),
        Err(ProofSessionStructuralErrorV3::Io)
    ));
    assert!(io.slots[17].is_none());

    let mut complete = ProofReplayPermitsV3::new_v3();
    for purpose in REPLAY_PURPOSES_V3 {
        let permit = complete
            .take_for_test_v3(purpose, TestReplayOutcomeV3::Success)
            .unwrap();
        complete.complete_v3(permit).unwrap();
    }
    assert_eq!(complete.completion_mask, REPLAY_COMPLETION_MASK_V3);
    assert!(complete.finish_v3().is_ok());
}

struct TestRelationV3;

#[test]
fn session_moves_through_typestates_and_destroys_on_every_failure() {
    TEST_OWNED_SIBLING_DROPS_V3.store(0, Ordering::SeqCst);
    let session: GlobalLookupProofSessionV3<TestRelationV3, PreZOpenings> =
        GlobalLookupProofSessionV3::test_only_v3([1; 32]);
    let session: GlobalLookupProofSessionV3<_, SoleZLive> = session
        .advance_for_test_v3(TestReplayOutcomeV3::Success)
        .unwrap();
    let session: GlobalLookupProofSessionV3<_, PostZBound> = session
        .advance_for_test_v3(TestReplayOutcomeV3::Success)
        .unwrap();
    let session: GlobalLookupProofSessionV3<_, PostDeltaResiduals> = session
        .advance_for_test_v3(TestReplayOutcomeV3::Success)
        .unwrap();
    let session: GlobalLookupProofSessionV3<_, RetainedOpenings> = session
        .advance_for_test_v3(TestReplayOutcomeV3::Success)
        .unwrap();
    let session: GlobalLookupProofSessionV3<_, ExistingSumcheckComplete> = session
        .advance_for_test_v3(TestReplayOutcomeV3::Success)
        .unwrap();
    let session: GlobalLookupProofSessionV3<_, TensorChallenges> = session
        .advance_for_test_v3(TestReplayOutcomeV3::Success)
        .unwrap();
    let session: GlobalLookupProofSessionV3<_, TensorSumcheckComplete> = session
        .advance_for_test_v3(TestReplayOutcomeV3::Success)
        .unwrap();
    let session: GlobalLookupProofSessionV3<_, EndpointsBound> = session
        .advance_for_test_v3(TestReplayOutcomeV3::Success)
        .unwrap();
    let session: GlobalLookupProofSessionV3<_, OpeningsBound> = session
        .advance_for_test_v3(TestReplayOutcomeV3::Success)
        .unwrap();
    let session: GlobalLookupProofSessionV3<_, Verified> = session
        .advance_for_test_v3(TestReplayOutcomeV3::Success)
        .unwrap();
    assert!(session.live.is_some() && !session.poisoned);
    drop(session);

    let valid = EntropyCoordinateV3::new_v3(
        EntropyPhaseV3::TensorSumcheck,
        EntropyPurposeV3::ChallengeRejection,
        0,
        0,
    )
    .unwrap();
    let invalid = EntropyCoordinateV3 {
        retry: 128,
        ..valid
    };
    for (coordinate, outcome, expected) in [
        (
            invalid,
            TestReplayOutcomeV3::Success,
            ProofSessionStructuralErrorV3::Entropy,
        ),
        (
            valid,
            TestReplayOutcomeV3::ValidationError,
            ProofSessionStructuralErrorV3::Validation,
        ),
        (
            valid,
            TestReplayOutcomeV3::IoError,
            ProofSessionStructuralErrorV3::Io,
        ),
    ] {
        let session =
            GlobalLookupProofSessionV3::<TestRelationV3, PreZOpenings>::test_only_v3([2; 32]);
        assert!(matches!(session.replay_once_for_test_v3(
            ProofReplayPurposeV3::TensorTermRound1, coordinate, outcome), Err(error) if error == expected));
    }
    let unwind = std::panic::catch_unwind(|| {
        let session =
            GlobalLookupProofSessionV3::<TestRelationV3, PreZOpenings>::test_only_v3([3; 32]);
        let _ = session.replay_once_for_test_v3(
            ProofReplayPurposeV3::TensorTermRound2,
            valid,
            TestReplayOutcomeV3::Unwind,
        );
    });
    assert!(unwind.is_err());
    let session = GlobalLookupProofSessionV3::<TestRelationV3, PreZOpenings>::test_only_v3([4; 32]);
    assert!(matches!(
        session.advance_for_test_v3::<SoleZLive>(TestReplayOutcomeV3::ValidationError),
        Err(ProofSessionStructuralErrorV3::Validation)
    ));
    let unwind = std::panic::catch_unwind(|| {
        let session =
            GlobalLookupProofSessionV3::<TestRelationV3, PreZOpenings>::test_only_v3([5; 32]);
        let _ = session.advance_for_test_v3::<SoleZLive>(TestReplayOutcomeV3::Unwind);
    });
    assert!(unwind.is_err());
    assert_eq!(TEST_OWNED_SIBLING_DROPS_V3.load(Ordering::SeqCst), 7);
}

#[test]
fn source_surface_is_private_uninhabited_and_gate_inert() {
    const SOURCE: &str = include_str!("global_lookup_proof_session_v3.rs");
    const PARENT: &str = include_str!("../vector_arithmetic_plane_openings_v1.rs");
    assert!(has_exact_private_module_declaration_v3(PARENT));
    assert!(has_exact_private_module_declaration_v3(
        "#[path = \"child.rs\"]\nmod global_lookup_proof_session_v3;"
    ));
    assert!(has_exact_private_module_declaration_v3(
        "#[path = r#\"child.rs\"#]\nmod\nglobal_lookup_proof_session_v3\n;"
    ));
    for visible_declaration in [
        "pub mod global_lookup_proof_session_v3;",
        "pub(crate) mod global_lookup_proof_session_v3;",
        "pub(crate)\nmod global_lookup_proof_session_v3;",
        "pub(\ncrate\n)\nmod global_lookup_proof_session_v3;",
        "pub(super) mod global_lookup_proof_session_v3;",
        "pub(super)\nmod global_lookup_proof_session_v3;",
        "pub(\nsuper\n)\nmod global_lookup_proof_session_v3;",
        "pub(in crate) mod global_lookup_proof_session_v3;",
        "pub(in crate)\nmod global_lookup_proof_session_v3;",
        "pub(\nin\ncrate\n)\nmod global_lookup_proof_session_v3;",
        "#[path = \"child.rs\"]\npub /* visible */ (crate)\nmod global_lookup_proof_session_v3;",
    ] {
        assert!(!has_exact_private_module_declaration_v3(
            visible_declaration
        ));
    }
    for lexical_decoy in [
        "// mod global_lookup_proof_session_v3;",
        "/* mod global_lookup_proof_session_v3; */",
        "const NOTE: &str = \"mod global_lookup_proof_session_v3;\";",
        "const NOTE: &str = r#\"mod global_lookup_proof_session_v3;\"#;",
    ] {
        assert!(!has_exact_private_module_declaration_v3(lexical_decoy));
    }
    assert!(!SOURCE.contains("V1"));
    for forbidden in [
        "global_z_rendezvous",
        "commitment_session",
        "source_openings",
        "Deref",
        "Fn(",
        "FnMut",
        "FnOnce",
        "callback",
        "decompos",
        "-> (",
        "fn snapshot",
        "fn path",
        "fn key",
        "fn rng",
        "fn get_",
    ] {
        assert!(
            !SOURCE.contains(forbidden),
            "forbidden surface: {forbidden}"
        );
    }
    assert!(has_exact_private_session_declaration_v3(SOURCE));
    const SESSION_BODY: &str = " {\n    live: Option<GlobalLookupProofSessionLiveV3<R>>,\n    poisoned: bool,\n    state: PhantomData<State>,\n}";
    let session_prefix =
        "#[must_use = \"dropping the session destroys its sole live structural owner\"]\n";
    const BRACE_LITERALS_BEFORE: &str = r###"
const CLOSE: char = '}';
const CLOSE_BYTE: u8 = b'}';
const QUOTE: char = '\'';
const QUOTE_BYTE: u8 = b'\'';
const BACKSLASH: char = '\\';
const BACKSLASH_BYTE: u8 = b'\\';
const NEWLINE: char = '\n';
const HEX_OPEN: char = '\x7b';
const HEX_CLOSE_BYTE: u8 = b'\x7d';
const UNICODE_OPEN: char = '\u{7b}';
"###;
    const BRACE_LITERALS_AFTER: &str = r###"
const OPEN: char = '{';
const OPEN_BYTE: u8 = b'{';
"###;
    let nested_char_decoy = [
        "fn nested<'a>() {\nconst CLOSE: char = '}';\n",
        "mod global_lookup_proof_session_v3;\n",
        session_prefix,
        "struct GlobalLookupProofSessionV3<R, State>",
        SESSION_BODY,
        "\nconst OPEN: char = '{';\n'label: loop { break 'label; }\n",
        "let _: Option<&'a str> = None;\n}\n",
    ]
    .concat();
    assert!(!has_exact_private_module_declaration_v3(&nested_char_decoy));
    assert!(!has_exact_private_session_declaration_v3(
        &nested_char_decoy
    ));
    assert!(has_exact_private_module_declaration_v3(
        &[
            BRACE_LITERALS_BEFORE,
            "mod global_lookup_proof_session_v3;",
            BRACE_LITERALS_AFTER,
        ]
        .concat()
    ));
    assert!(has_exact_private_session_declaration_v3(
        &[
            BRACE_LITERALS_BEFORE,
            session_prefix,
            "struct GlobalLookupProofSessionV3<R, State>",
            SESSION_BODY,
            BRACE_LITERALS_AFTER,
        ]
        .concat()
    ));
    assert!(has_exact_private_session_declaration_v3(
        "#[must_use\n=\n\"dropping the session destroys its sole live structural owner\"]\n\
         struct\nGlobalLookupProofSessionV3\n<\nR,\nState\n>\n{\n\
         live:\nOption<GlobalLookupProofSessionLiveV3<R>>,\n\
         poisoned:\nbool,\nstate:\nPhantomData<State>,\n}"
    ));
    assert!(!has_exact_private_session_declaration_v3(&format!(
        "{session_prefix}pub(crate) struct GlobalLookupProofSessionV3<R, State>{SESSION_BODY}"
    )));
    assert!(!has_exact_private_session_declaration_v3(&format!(
        "#[derive(Clone, Copy)]\n{session_prefix}struct GlobalLookupProofSessionV3<R, State>{SESSION_BODY}"
    )));
    assert!(!has_exact_private_session_declaration_v3(&format!(
        "/* {session_prefix}struct GlobalLookupProofSessionV3<R, State>{SESSION_BODY} */\n\
         {session_prefix}pub(\ncrate\n)\nstruct GlobalLookupProofSessionV3<R, State>{SESSION_BODY}"
    )));
    assert!(!has_exact_private_session_declaration_v3(&format!(
        "const DECOY: &str = r###\"{session_prefix}struct GlobalLookupProofSessionV3<R, State> {{}}\"###;\n\
         {session_prefix}pub(\nsuper\n)\nstruct GlobalLookupProofSessionV3\n<\nR,\nState\n>{SESSION_BODY}"
    )));
    assert!(!has_exact_private_session_declaration_v3(&format!(
        "#[derive(\nClone,\nCopy,\nDebug,\nDefault,\nDeref\n)]\n\
         {session_prefix}struct GlobalLookupProofSessionV3<R, State>{SESSION_BODY}"
    )));
    assert!(!has_exact_private_session_declaration_v3(
        "const NOTE: &str = \"struct GlobalLookupProofSessionV3<R, State> {\";"
    ));
    for field in [
        "entropy: Infallible",
        "source: Infallible",
        "component: Infallible",
        "materializer: Infallible",
        "rehome_existing_owners: Infallible",
    ] {
        assert!(SOURCE.contains(field), "missing production seal: {field}");
    }
    assert!(SOURCE.contains("slots: [Option<ProofReplayPermitV3>; 18]"));
    let replay = SOURCE.split("fn replay_once_for_test_v3").nth(1).unwrap();
    let poison = replay.find("self.poisoned = true;").unwrap();
    let take = replay.find(".live\n            .take()").unwrap();
    let remove = replay.find("live.permits.remove_v3(purpose)?").unwrap();
    let validate = replay
        .find("coordinate.retry > MAX_ENTROPY_RETRY_V3")
        .unwrap();
    let io = replay.find("match outcome").unwrap();
    assert!(poison < take && take < remove && remove < validate && validate < io);
    assert!(!PRODUCTION_ENTROPY_INHABITED_V3 && !PRODUCTION_SOURCE_INHABITED_V3);
    assert!(!PRODUCTION_MATERIALIZER_INHABITED_V3 && !PRODUCTION_TRANSITIONS_INHABITED_V3);
    assert!(!PROOF_SESSION_WIRED_V3 && !PROOF_VERIFIED_V3 && !ZERO_KNOWLEDGE_ACCEPTED_V3);
    assert!(!COMPLETE_ACCOUNTING_QUALIFIED_V3 && !RSS_QUALIFIED_V3);
    assert!(!READINESS_QUALIFIED_V3 && !OPERATIONAL_RECEIPT_ACCEPTED_V3);
    assert!(!AUTHORITY_MINTED_V3 && !RELEASE_READY_V3 && !RELEASE_COMPLETE_V3);
}
