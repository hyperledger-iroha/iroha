use super::*;

const RUST_ORACLE_MAX_BYTES_V1: usize = 256 * 1_024;
const RUST_ORACLE_MAX_DEPTH_V1: usize = 256;

#[derive(Clone, Copy, PartialEq, Eq)]
enum RustTokenV1<'source> {
    Word(&'source str),
    Number(&'source str),
    Literal(&'source str),
    Punct(u8),
}

fn rust_tokens_v1(source: &str) -> Option<Vec<RustTokenV1<'_>>> {
    use RustTokenV1::{Literal, Number, Punct, Word};

    fn raw_prefix(bytes: &[u8], offset: usize) -> Option<(usize, usize)> {
        let mut cursor = offset;
        if matches!(bytes.get(cursor), Some(b'b' | b'c')) {
            cursor += 1;
        }
        if bytes.get(cursor) != Some(&b'r') {
            return None;
        }
        cursor += 1;
        let hashes = cursor;
        while bytes.get(cursor) == Some(&b'#') {
            cursor += 1;
        }
        (bytes.get(cursor) == Some(&b'"') && cursor - hashes <= usize::from(u8::MAX))
            .then_some((cursor, cursor - hashes))
    }

    fn char_end(source: &str, offset: usize) -> Option<usize> {
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
                    let (mut digits, mut scalar) = (0_u8, 0_u32);
                    while bytes.get(cursor) != Some(&b'}') {
                        let byte = *bytes.get(cursor)?;
                        if byte != b'_' {
                            digits = digits.checked_add(1)?;
                            scalar = scalar
                                .checked_mul(16)?
                                .checked_add((byte as char).to_digit(16)?)?;
                        }
                        if digits > 6 {
                            return None;
                        }
                        cursor += 1;
                    }
                    if digits == 0 || char::from_u32(scalar).is_none() {
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

    if source.len() > RUST_ORACLE_MAX_BYTES_V1 {
        return None;
    }
    let bytes = source.as_bytes();
    let mut tokens = Vec::new();
    let mut delimiters = Vec::new();
    let mut offset = 0;
    while offset < bytes.len() {
        if bytes[offset].is_ascii_whitespace() {
            offset += 1;
            continue;
        }
        if bytes[offset..].starts_with(b"//") {
            offset += 2;
            while bytes.get(offset).is_some_and(|byte| *byte != b'\n') {
                offset += 1;
            }
            continue;
        }
        if bytes[offset..].starts_with(b"/*") {
            offset += 2;
            let mut depth = 1_usize;
            while depth != 0 {
                if bytes.get(offset..offset + 2) == Some(b"/*") {
                    depth = depth.checked_add(1)?;
                    if depth > RUST_ORACLE_MAX_DEPTH_V1 {
                        return None;
                    }
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
        let start = offset;
        let token = if let Some((quote, hashes)) = raw_prefix(bytes, offset) {
            offset = quote + 1;
            loop {
                offset += bytes.get(offset..)?.iter().position(|byte| *byte == b'"')? + 1;
                if bytes.get(offset..offset + hashes) == Some(&bytes[quote - hashes..quote]) {
                    offset += hashes;
                    break;
                }
            }
            Literal(&source[start..offset])
        } else if bytes[offset] == b'"' {
            offset += 1;
            loop {
                match *bytes.get(offset)? {
                    b'\\' => {
                        offset = offset.checked_add(2).filter(|next| *next <= bytes.len())?;
                    }
                    b'"' => {
                        offset += 1;
                        break;
                    }
                    _ => offset += 1,
                }
            }
            Literal(&source[start..offset])
        } else if let Some(end) = char_end(source, offset) {
            offset = end;
            Literal(&source[start..offset])
        } else if bytes[offset].is_ascii_alphabetic() || bytes[offset] == b'_' {
            offset += 1;
            while bytes
                .get(offset)
                .is_some_and(|byte| byte.is_ascii_alphanumeric() || *byte == b'_')
            {
                offset += 1;
            }
            Word(&source[start..offset])
        } else if bytes[offset].is_ascii_digit() {
            offset += 1;
            while bytes
                .get(offset)
                .is_some_and(|byte| byte.is_ascii_alphanumeric() || *byte == b'_')
            {
                offset += 1;
            }
            Number(&source[start..offset])
        } else if !bytes[offset].is_ascii() {
            return None;
        } else {
            let punct = bytes[offset];
            offset += 1;
            match punct {
                b'(' | b'[' | b'{' => {
                    if delimiters.len() == RUST_ORACLE_MAX_DEPTH_V1 {
                        return None;
                    }
                    delimiters.push(punct);
                }
                b')' if delimiters.pop() != Some(b'(') => return None,
                b']' if delimiters.pop() != Some(b'[') => return None,
                b'}' if delimiters.pop() != Some(b'{') => return None,
                _ => {}
            }
            Punct(punct)
        };
        if tokens.len() == RUST_ORACLE_MAX_BYTES_V1 {
            return None;
        }
        tokens.push(token);
    }
    delimiters.is_empty().then_some(tokens)
}

fn matching_pair_v1(
    tokens: &[RustTokenV1<'_>],
    start: usize,
    open: u8,
    close: u8,
) -> Option<usize> {
    use RustTokenV1::Punct;
    (tokens.get(start) == Some(&Punct(open))).then_some(())?;
    let mut depth = 0_usize;
    for (index, token) in tokens.iter().enumerate().skip(start) {
        match token {
            Punct(value) if *value == open => depth += 1,
            Punct(value) if *value == close => {
                depth -= 1;
                if depth == 0 {
                    return Some(index);
                }
            }
            _ => {}
        }
    }
    None
}

fn outer_attributes_v1<'tokens, 'source>(
    tokens: &'tokens [RustTokenV1<'source>],
) -> Option<Vec<&'tokens [RustTokenV1<'source>]>> {
    use RustTokenV1::Punct;
    let (mut cursor, mut attributes) = (0, Vec::new());
    while cursor < tokens.len() {
        if tokens.get(cursor) != Some(&Punct(b'#')) || tokens.get(cursor + 1) != Some(&Punct(b'['))
        {
            return None;
        }
        let end = matching_pair_v1(tokens, cursor + 1, b'[', b']')?;
        attributes.push(&tokens[cursor + 2..end]);
        cursor = end + 1;
    }
    Some(attributes)
}

fn string_literal_eq_v1(literal: &str, expected: &str) -> bool {
    if let Some(cooked) = literal
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
    {
        return !cooked.contains('\\') && cooked == expected;
    }
    let Some(raw) = literal.strip_prefix('r') else {
        return false;
    };
    let hashes = raw.bytes().take_while(|byte| *byte == b'#').count();
    let Some(body) = raw.get(hashes..).and_then(|value| value.strip_prefix('"')) else {
        return false;
    };
    let suffix = "#".repeat(hashes);
    let Some(body) = body.strip_suffix(suffix.as_str()) else {
        return false;
    };
    body.strip_suffix('"') == Some(expected)
}

fn exact_private_path_module_v1(source: &str, name: &str, path: &str) -> bool {
    use RustTokenV1::{Literal, Punct, Word};
    let Some(tokens) = rust_tokens_v1(source) else {
        return false;
    };
    let (mut depth, mut item_start, mut candidates, mut exact) = (0, 0, 0, false);
    for index in 0..tokens.len() {
        if tokens.get(index) == Some(&Word("mod"))
            && tokens.get(index + 1) == Some(&Word(name))
            && tokens.get(index + 2) == Some(&Punct(b';'))
        {
            candidates += 1;
            if depth == 0 {
                let Some(attributes) = outer_attributes_v1(&tokens[item_start..index]) else {
                    return false;
                };
                let mut matching_paths = 0;
                let mut attributes_are_production_safe = true;
                for attribute in attributes {
                    match attribute {
                        [Word("path"), Punct(b'='), Literal(value)]
                            if string_literal_eq_v1(value, path) =>
                        {
                            matching_paths += 1;
                        }
                        [Word("path"), ..] | [Word("cfg" | "cfg_attr"), ..] => {
                            attributes_are_production_safe = false;
                        }
                        _ => {}
                    }
                }
                exact = attributes_are_production_safe && matching_paths == 1;
            }
        }
        match tokens[index] {
            Punct(b'(' | b'[' | b'{') => depth += 1,
            Punct(b')' | b']' | b'}') => {
                depth -= 1;
                if depth == 0 && tokens[index] == Punct(b'}') {
                    item_start = index + 1;
                }
            }
            Punct(b';') if depth == 0 => item_start = index + 1,
            _ => {}
        }
    }
    candidates == 1 && exact
}

fn contains_no_words_v1(source: &str, forbidden: &[&str]) -> bool {
    let Some(tokens) = rust_tokens_v1(source) else {
        return false;
    };
    !tokens
        .iter()
        .any(|token| matches!(token, RustTokenV1::Word(word) if forbidden.contains(word)))
}

fn exact_word_count_v1(source: &str, word: &str, expected: usize) -> bool {
    rust_tokens_v1(source).is_some_and(|tokens| {
        tokens
            .iter()
            .filter(|token| **token == RustTokenV1::Word(word))
            .count()
            == expected
    })
}

type AssociatedFunctionPartsV1 = (
    core::ops::Range<usize>,
    core::ops::Range<usize>,
    core::ops::Range<usize>,
    core::ops::Range<usize>,
    core::ops::Range<usize>,
);

fn associated_function_parts_v1(
    tokens: &[RustTokenV1<'_>],
    function: usize,
) -> Option<AssociatedFunctionPartsV1> {
    use RustTokenV1::{Punct, Word};
    matches!(tokens.get(function), Some(Word("fn"))).then_some(())?;
    matches!(tokens.get(function + 1), Some(Word(_))).then_some(())?;

    let mut cursor = function + 2;
    let mut angles = 0_usize;
    let parameters = loop {
        match tokens.get(cursor)? {
            Punct(b'<') => angles = angles.checked_add(1)?,
            Punct(b'>')
                if angles != 0 && tokens.get(cursor.wrapping_sub(1)) != Some(&Punct(b'-')) =>
            {
                angles -= 1;
            }
            Punct(b'(') if angles == 0 => break cursor,
            Punct(open @ (b'(' | b'[' | b'{')) => {
                let close = match *open {
                    b'(' => b')',
                    b'[' => b']',
                    b'{' => b'}',
                    _ => unreachable!(),
                };
                cursor = matching_pair_v1(tokens, cursor, *open, close)?;
            }
            Punct(b';' | b'}') => return None,
            _ => {}
        }
        cursor += 1;
    };
    let parameters_end = matching_pair_v1(tokens, parameters, b'(', b')')?;
    cursor = parameters_end + 1;
    let return_start = (tokens.get(cursor) == Some(&Punct(b'-'))
        && tokens.get(cursor + 1) == Some(&Punct(b'>')))
    .then_some(cursor + 2);
    if return_start.is_some() {
        cursor += 2;
    }
    angles = 0;
    let mut where_start = None;
    let body = loop {
        match tokens.get(cursor)? {
            Word("where") if angles == 0 => {
                where_start.get_or_insert(cursor);
            }
            Punct(b'<') => {
                angles = angles.checked_add(1)?;
            }
            Punct(b'>')
                if angles != 0 && tokens.get(cursor.wrapping_sub(1)) != Some(&Punct(b'-')) =>
            {
                angles -= 1;
            }
            Punct(open @ (b'(' | b'[')) => {
                let close = if *open == b'(' { b')' } else { b']' };
                cursor = matching_pair_v1(tokens, cursor, *open, close)?;
            }
            Punct(b'{') if angles != 0 => {
                cursor = matching_pair_v1(tokens, cursor, b'{', b'}')?;
            }
            Punct(b'{') => break cursor,
            Punct(b';' | b'}') => return None,
            _ => {}
        }
        cursor += 1;
    };
    let body_end = matching_pair_v1(tokens, body, b'{', b'}')?;
    let generics = function + 2..parameters;
    let parameters = parameters + 1..parameters_end;
    let returns = return_start.map_or(body..body, |start| start..where_start.unwrap_or(body));
    let bounds = where_start.map_or(body..body, |start| start + 1..body);
    Some((generics, parameters, returns, bounds, body + 1..body_end))
}

fn direct_context_mints_v1(tokens: &[RustTokenV1<'_>], context: &str) -> usize {
    use RustTokenV1::{Punct, Word};
    tokens
        .windows(2)
        .filter(|pair| {
            let pair = *pair;
            let owner = matches!(pair.first(), Some(Word("Self")))
                || matches!(pair.first(), Some(Word(word)) if *word == context);
            owner && matches!(pair.get(1), Some(Punct(b'{' | b':')))
        })
        .count()
}

fn top_level_item_ranges_v1(tokens: &[RustTokenV1<'_>]) -> Option<Vec<core::ops::Range<usize>>> {
    use RustTokenV1::Punct;
    let (mut ranges, mut start, mut depth) = (Vec::new(), 0, 0_usize);
    for (index, token) in tokens.iter().enumerate() {
        match token {
            Punct(b'(' | b'[' | b'{') => depth = depth.checked_add(1)?,
            Punct(b')' | b']' | b'}') => {
                depth = depth.checked_sub(1)?;
                if depth == 0 && *token == Punct(b'}') {
                    if start < index + 1 {
                        ranges.push(start..index + 1);
                    }
                    start = index + 1;
                }
            }
            Punct(b';') if depth == 0 => {
                if start < index {
                    ranges.push(start..index + 1);
                }
                start = index + 1;
            }
            _ => {}
        }
    }
    (depth == 0 && start == tokens.len()).then_some(ranges)
}

fn leading_outer_attributes_v1<'tokens, 'source>(
    tokens: &'tokens [RustTokenV1<'source>],
) -> Option<(Vec<&'tokens [RustTokenV1<'source>]>, usize)> {
    use RustTokenV1::Punct;
    let (mut cursor, mut attributes) = (0, Vec::new());
    while tokens.get(cursor) == Some(&Punct(b'#')) {
        if tokens.get(cursor + 1) != Some(&Punct(b'[')) {
            return None;
        }
        let end = matching_pair_v1(tokens, cursor + 1, b'[', b']')?;
        attributes.push(&tokens[cursor + 2..end]);
        cursor = end + 1;
    }
    Some((attributes, cursor))
}

fn item_is_exact_cfg_test_v1(tokens: &[RustTokenV1<'_>]) -> bool {
    use RustTokenV1::{Punct, Word};
    let Some((attributes, _)) = leading_outer_attributes_v1(tokens) else {
        return false;
    };
    let exact = [Word("cfg"), Punct(b'('), Word("test"), Punct(b')')];
    !attributes
        .iter()
        .any(|attribute| attribute.first() == Some(&Word("cfg_attr")))
        && attributes
            .iter()
            .filter(|attribute| attribute.first() == Some(&Word("cfg")))
            .copied()
            .eq(core::iter::once(exact.as_slice()))
}

fn top_level_word_v1(tokens: &[RustTokenV1<'_>], expected: &str) -> Option<usize> {
    use RustTokenV1::{Punct, Word};
    let mut depth = 0_usize;
    for (index, token) in tokens.iter().enumerate() {
        if depth == 0 && *token == Word(expected) {
            return Some(index);
        }
        match token {
            Punct(b'(' | b'[' | b'{') => depth = depth.checked_add(1)?,
            Punct(b')' | b']' | b'}') => depth = depth.checked_sub(1)?,
            _ => {}
        }
    }
    None
}

fn dangerous_context_word_v1(
    token: &RustTokenV1<'_>,
    context: &str,
    self_is_context: bool,
) -> bool {
    matches!(token, RustTokenV1::Word(word) if *word == context)
        || (self_is_context && *token == RustTokenV1::Word("Self"))
}

fn has_dangerous_context_word_v1(
    tokens: &[RustTokenV1<'_>],
    context: &str,
    self_is_context: bool,
) -> bool {
    tokens
        .iter()
        .any(|token| dangerous_context_word_v1(token, context, self_is_context))
}

fn comma_separated_ranges_v1(tokens: &[RustTokenV1<'_>]) -> Option<Vec<core::ops::Range<usize>>> {
    use RustTokenV1::Punct;
    let (mut ranges, mut start, mut depth, mut angles) = (Vec::new(), 0, 0_usize, 0_usize);
    for (index, token) in tokens.iter().enumerate() {
        match token {
            Punct(b'(' | b'[' | b'{') => depth = depth.checked_add(1)?,
            Punct(b')' | b']' | b'}') => depth = depth.checked_sub(1)?,
            Punct(b'<') if depth == 0 => angles = angles.checked_add(1)?,
            Punct(b'>')
                if depth == 0
                    && angles != 0
                    && tokens.get(index.wrapping_sub(1)) != Some(&Punct(b'-')) =>
            {
                angles -= 1;
            }
            Punct(b',') if depth == 0 && angles == 0 => {
                if start < index {
                    ranges.push(start..index);
                }
                start = index + 1;
            }
            _ => {}
        }
    }
    if depth != 0 || angles != 0 {
        return None;
    }
    if start < tokens.len() {
        ranges.push(start..tokens.len());
    }
    Some(ranges)
}

fn exact_read_only_context_input_v1(
    tokens: &[RustTokenV1<'_>],
    context: &str,
    self_is_context: bool,
) -> bool {
    use RustTokenV1::{Punct, Word};
    let dangerous =
        |token: &RustTokenV1<'_>| dangerous_context_word_v1(token, context, self_is_context);
    if matches!(tokens, [only] if dangerous(only)) {
        return true;
    }
    match tokens {
        [Punct(b'&'), owner] if dangerous(owner) => true,
        [Punct(b'&'), Punct(b'\''), Word(_), owner] if dangerous(owner) => true,
        _ => false,
    }
}

fn parameters_are_context_inputs_only_v1(
    tokens: &[RustTokenV1<'_>],
    context: &str,
    self_is_context: bool,
) -> bool {
    use RustTokenV1::Punct;
    let Some(parameters) = comma_separated_ranges_v1(tokens) else {
        return false;
    };
    for parameter in parameters {
        let parameter = &tokens[parameter];
        if !has_dangerous_context_word_v1(parameter, context, self_is_context) {
            continue;
        }
        let (mut depth, mut angles, mut colon) = (0_usize, 0_usize, None);
        for (index, token) in parameter.iter().enumerate() {
            match token {
                Punct(b'(' | b'[' | b'{') => {
                    let Some(next) = depth.checked_add(1) else {
                        return false;
                    };
                    depth = next;
                }
                Punct(b')' | b']' | b'}') => {
                    let Some(next) = depth.checked_sub(1) else {
                        return false;
                    };
                    depth = next;
                }
                Punct(b'<') if depth == 0 => {
                    let Some(next) = angles.checked_add(1) else {
                        return false;
                    };
                    angles = next;
                }
                Punct(b'>') if depth == 0 && angles != 0 => angles -= 1,
                Punct(b':') if depth == 0 && angles == 0 => {
                    if colon.replace(index).is_some() {
                        return false;
                    }
                }
                _ => {}
            }
        }
        let Some(colon) = colon else {
            return false;
        };
        if !exact_read_only_context_input_v1(&parameter[colon + 1..], context, self_is_context) {
            return false;
        }
    }
    true
}

fn bounds_use_context_as_input_only_v1(
    tokens: &[RustTokenV1<'_>],
    context: &str,
    self_is_context: bool,
) -> bool {
    use RustTokenV1::{Punct, Word};
    let Some(clauses) = comma_separated_ranges_v1(tokens) else {
        return false;
    };
    for clause in clauses {
        let clause = &tokens[clause];
        for (index, token) in clause.iter().enumerate() {
            if !dangerous_context_word_v1(token, context, self_is_context) {
                continue;
            }
            if *token == Word("Self") && clause.get(index + 1) == Some(&Punct(b':')) {
                continue;
            }
            let reference = if index != 0 && clause[index - 1] == Punct(b'&') {
                index - 1
            } else if index >= 3
                && matches!(clause[index - 1], Word(_))
                && clause[index - 2] == Punct(b'\'')
                && clause[index - 3] == Punct(b'&')
            {
                index - 3
            } else {
                return false;
            };
            if clause[..reference]
                .windows(2)
                .any(|pair| pair == [Punct(b'-'), Punct(b'>')])
            {
                return false;
            }
        }
    }
    true
}

fn nested_item_end_v1(
    tokens: &[RustTokenV1<'_>],
    start: usize,
    brace_terminated: bool,
) -> Option<usize> {
    use RustTokenV1::Punct;
    let mut depth = 0_usize;
    for (index, token) in tokens.iter().enumerate().skip(start) {
        match token {
            Punct(b'(' | b'[' | b'{') => {
                depth = depth.checked_add(1)?;
                if depth > RUST_ORACLE_MAX_DEPTH_V1 {
                    return None;
                }
            }
            Punct(b')' | b']' | b'}') => {
                depth = depth.checked_sub(1)?;
                if brace_terminated && depth == 0 && *token == Punct(b'}') {
                    return Some(index + 1);
                }
            }
            Punct(b';') if depth == 0 => return Some(index + 1),
            _ => {}
        }
    }
    None
}

fn exact_cfg_test_nested_item_end_v1(tokens: &[RustTokenV1<'_>], start: usize) -> Option<usize> {
    use RustTokenV1::{Punct, Word};
    const ITEM_WORDS: [&str; 14] = [
        "fn",
        "const",
        "static",
        "type",
        "macro_rules",
        "macro",
        "trait",
        "impl",
        "mod",
        "struct",
        "enum",
        "union",
        "use",
        "extern",
    ];
    let suffix = tokens.get(start..)?;
    if !item_is_exact_cfg_test_v1(suffix) {
        return None;
    }
    let (_, attributes_end) = leading_outer_attributes_v1(suffix)?;
    let mut keyword = None;
    let mut depth = 0_usize;
    for (index, token) in tokens.iter().enumerate().skip(start + attributes_end) {
        if depth == 0 {
            if let Word(word) = token
                && ITEM_WORDS.contains(word)
                && (keyword.is_none() || *word == "fn")
            {
                keyword = Some(*word);
            }
            if *token == Punct(b';') {
                return keyword.is_some().then_some(index + 1);
            }
            if *token == Punct(b'{') {
                let keyword = keyword?;
                let brace_terminated = matches!(
                    keyword,
                    "fn" | "macro_rules"
                        | "macro"
                        | "trait"
                        | "impl"
                        | "mod"
                        | "struct"
                        | "enum"
                        | "union"
                        | "extern"
                );
                return nested_item_end_v1(tokens, start, brace_terminated);
            }
        }
        match token {
            Punct(b'(' | b'[' | b'{') => depth = depth.checked_add(1)?,
            Punct(b')' | b']' | b'}') => depth = depth.checked_sub(1)?,
            _ => {}
        }
    }
    None
}

fn macro_group_v1(tokens: &[RustTokenV1<'_>], bang: usize) -> Option<core::ops::Range<usize>> {
    use RustTokenV1::{Punct, Word};
    (tokens.get(bang) == Some(&Punct(b'!'))).then_some(())?;
    let group = if matches!(tokens.get(bang + 1), Some(Punct(b'(' | b'[' | b'{'))) {
        bang + 1
    } else if tokens.get(bang.wrapping_sub(1)) == Some(&Word("macro_rules"))
        && matches!(tokens.get(bang + 1), Some(Word(_)))
        && matches!(tokens.get(bang + 2), Some(Punct(b'(' | b'[' | b'{')))
    {
        bang + 2
    } else {
        return None;
    };
    let (open, close) = match tokens[group] {
        Punct(b'(') => (b'(', b')'),
        Punct(b'[') => (b'[', b']'),
        Punct(b'{') => (b'{', b'}'),
        _ => return None,
    };
    let end = matching_pair_v1(tokens, group, open, close)?;
    Some(group + 1..end)
}

fn nested_function_is_context_safe_v1(
    tokens: &[RustTokenV1<'_>],
    function: usize,
    context: &str,
    self_is_context: bool,
) -> Option<usize> {
    let (generics, parameters, returns, bounds, body) =
        associated_function_parts_v1(tokens, function)?;
    let end = body.end + 1;
    (!has_dangerous_context_word_v1(&tokens[generics], context, self_is_context)
        && parameters_are_context_inputs_only_v1(&tokens[parameters], context, self_is_context)
        && !has_dangerous_context_word_v1(&tokens[returns], context, self_is_context)
        && bounds_use_context_as_input_only_v1(&tokens[bounds], context, self_is_context)
        && function_body_has_no_context_mint_v1(&tokens[body], context, self_is_context))
    .then_some(end)
}

fn function_body_has_no_context_mint_v1(
    tokens: &[RustTokenV1<'_>],
    context: &str,
    self_is_context: bool,
) -> bool {
    use RustTokenV1::{Punct, Word};
    if !has_dangerous_context_word_v1(tokens, context, self_is_context) {
        return true;
    }
    let mut cursor = 0;
    while cursor < tokens.len() {
        if tokens.get(cursor) == Some(&Punct(b'#')) && item_is_exact_cfg_test_v1(&tokens[cursor..])
        {
            let Some(end) = exact_cfg_test_nested_item_end_v1(tokens, cursor) else {
                return false;
            };
            cursor = end;
            continue;
        }
        if tokens.get(cursor) == Some(&Punct(b'!'))
            && let Some(group) = macro_group_v1(tokens, cursor)
        {
            if has_dangerous_context_word_v1(&tokens[group.clone()], context, self_is_context) {
                return false;
            }
            cursor = group.end + 1;
            continue;
        }
        if tokens.get(cursor) == Some(&Word("fn"))
            && matches!(tokens.get(cursor + 1), Some(Word(_)))
        {
            let Some(end) =
                nested_function_is_context_safe_v1(tokens, cursor, context, self_is_context)
            else {
                return false;
            };
            cursor = end;
            continue;
        }
        if dangerous_context_word_v1(&tokens[cursor], context, self_is_context) {
            return false;
        }
        cursor += 1;
    }
    true
}

fn function_context_surface_is_safe_v1(
    tokens: &[RustTokenV1<'_>],
    function: usize,
    context: &str,
    self_is_context: bool,
) -> bool {
    !has_dangerous_context_word_v1(&tokens[..function], context, self_is_context)
        && nested_function_is_context_safe_v1(tokens, function, context, self_is_context).is_some()
}

fn impl_parts_v1(
    tokens: &[RustTokenV1<'_>],
    implementation: usize,
) -> Option<(core::ops::Range<usize>, core::ops::Range<usize>)> {
    use RustTokenV1::{Punct, Word};
    (tokens.get(implementation) == Some(&Word("impl"))).then_some(())?;
    let (mut cursor, mut angles) = (implementation + 1, 0_usize);
    let open = loop {
        match tokens.get(cursor)? {
            Punct(b'<') => angles = angles.checked_add(1)?,
            Punct(b'>')
                if angles != 0 && tokens.get(cursor.wrapping_sub(1)) != Some(&Punct(b'-')) =>
            {
                angles -= 1;
            }
            Punct(open @ (b'(' | b'[')) => {
                let close = if *open == b'(' { b')' } else { b']' };
                cursor = matching_pair_v1(tokens, cursor, *open, close)?;
            }
            Punct(b'{') if angles != 0 => {
                cursor = matching_pair_v1(tokens, cursor, b'{', b'}')?;
            }
            Punct(b'{') => break cursor,
            Punct(b';' | b'}') => return None,
            _ => {}
        }
        cursor += 1;
    };
    let close = matching_pair_v1(tokens, open, b'{', b'}')?;
    (close + 1 == tokens.len()).then_some((implementation + 1..open, open + 1..close))
}

fn context_impl_has_no_production_mint_v1(
    tokens: &[RustTokenV1<'_>],
    implementation: usize,
    context: &str,
) -> bool {
    use RustTokenV1::Word;
    let Some((header, body)) = impl_parts_v1(tokens, implementation) else {
        return false;
    };
    let self_is_context = tokens[header.clone()] == [Word(context)];
    if tokens[header.clone()].contains(&Word(context)) && !self_is_context {
        return false;
    }
    let Some(items) = top_level_item_ranges_v1(&tokens[body.clone()]) else {
        return false;
    };
    for item in items {
        let item = &tokens[body.start + item.start..body.start + item.end];
        if item_is_exact_cfg_test_v1(item) {
            continue;
        }
        let dangerous = has_dangerous_context_word_v1(item, context, self_is_context);
        if let Some(function) = top_level_word_v1(item, "fn") {
            if dangerous
                && !function_context_surface_is_safe_v1(item, function, context, self_is_context)
            {
                return false;
            }
        } else if dangerous {
            return false;
        }
    }
    true
}

fn exact_context_struct_item_v1(tokens: &[RustTokenV1<'_>], context: &str) -> bool {
    use RustTokenV1::{Punct, Word};
    let Some((_, attributes_end)) = leading_outer_attributes_v1(tokens) else {
        return false;
    };
    let Some(declaration) = top_level_word_v1(tokens, "struct") else {
        return false;
    };
    tokens[attributes_end..declaration] == [Word("pub"), Punct(b'('), Word("super"), Punct(b')')]
        && tokens.get(declaration + 1) == Some(&Word(context))
        && tokens.get(declaration + 2) == Some(&Punct(b'{'))
        && matching_pair_v1(tokens, declaration + 2, b'{', b'}') == Some(tokens.len() - 1)
        && tokens[declaration + 3..tokens.len() - 1]
            .iter()
            .all(|token| *token != Word("pub"))
        && tokens
            .iter()
            .filter(|token| **token == Word(context))
            .count()
            == 1
}

fn whole_module_context_mint_inventory_v1(source: &str) -> bool {
    use RustTokenV1::Word;
    const CONTEXT: &str = "ZkAmsPhase23RnsLinkContextV1";
    let Some(tokens) = rust_tokens_v1(source) else {
        return false;
    };
    let Some(items) = top_level_item_ranges_v1(&tokens) else {
        return false;
    };
    let mut context_struct = false;
    for item in items {
        let item = &tokens[item];
        if !item.contains(&Word(CONTEXT)) {
            continue;
        }
        if item_is_exact_cfg_test_v1(item) {
            continue;
        }
        if let Some(implementation) = top_level_word_v1(item, "impl") {
            if !context_impl_has_no_production_mint_v1(item, implementation, CONTEXT) {
                return false;
            }
        } else if top_level_word_v1(item, "struct").is_some() {
            if context_struct || !exact_context_struct_item_v1(item, CONTEXT) {
                return false;
            }
            context_struct = true;
        } else if let Some(function) = top_level_word_v1(item, "fn") {
            if !function_context_surface_is_safe_v1(item, function, CONTEXT, false) {
                return false;
            }
        } else {
            return false;
        }
    }
    context_struct
}

fn source_pin_v1(source: &str) -> (usize, [u8; 32]) {
    (
        source.len(),
        crate::vega::sponge::keccak256(source.as_bytes()),
    )
}

fn production_outer_attributes_are_inert_v1(attributes: &[&[RustTokenV1<'_>]]) -> bool {
    use RustTokenV1::{Punct, Word};
    const INERT_DERIVES: [&str; 8] = [
        "Clone",
        "Copy",
        "Debug",
        "Default",
        "PartialEq",
        "Eq",
        "PartialOrd",
        "Ord",
    ];
    attributes.iter().all(|attribute| match *attribute {
        [Word("allow" | "repr"), Punct(b'('), .., Punct(b')')]
        | [Word("path"), Punct(b'='), RustTokenV1::Literal(_)] => true,
        [Word("derive"), Punct(b'('), body @ .., Punct(b')')] => body.iter().all(|token| {
            matches!(token, Punct(b','))
                || matches!(token, Word(derive) if INERT_DERIVES.contains(derive))
        }),
        _ => false,
    })
}

fn has_production_expansion_surface_v1(tokens: &[RustTokenV1<'_>], invoked: &[&str]) -> bool {
    use RustTokenV1::{Punct, Word};
    for (index, token) in tokens.iter().enumerate() {
        if *token == Word("macro") {
            return true;
        }
        let Some(Word(name)) = index.checked_sub(1).and_then(|before| tokens.get(before)) else {
            continue;
        };
        if *token != Punct(b'!') || tokens.get(index + 1) == Some(&Punct(b'=')) {
            continue;
        }
        let exact_builtin = matches!(
            (*name, tokens.get(index + 1)),
            ("assert" | "panic" | "write", Some(Punct(b'('))) | ("vec", Some(Punct(b'[')))
        );
        if !exact_builtin
            || matches!(tokens.get(index.wrapping_sub(2)), Some(Punct(b'#' | b'$')))
            || (tokens.get(index.wrapping_sub(2)) == Some(&Punct(b':'))
                && tokens.get(index.wrapping_sub(3)) == Some(&Punct(b':')))
        {
            return true;
        }
    }
    for (index, token) in tokens.iter().enumerate() {
        if *token != Word("use") {
            continue;
        }
        let Some(end) = tokens[index..]
            .iter()
            .position(|token| *token == Punct(b';'))
        else {
            return true;
        };
        if tokens[index..index + end].iter().any(|token| {
            matches!(token, Word("include" | "include_bytes" | "include_str"))
                || matches!(token, Word(name) if invoked.contains(name))
        }) {
            return true;
        }
    }
    false
}

fn exact_production_child_modules_v1(source: &str, expected: &[(&str, &str)]) -> bool {
    use RustTokenV1::{Literal, Punct, Word};
    const CONTEXT: &str = "ZkAmsPhase23RnsLinkContextV1";
    let Some(tokens) = rust_tokens_v1(source) else {
        return false;
    };
    let Some(items) = top_level_item_ranges_v1(&tokens) else {
        return false;
    };
    let invoked = tokens
        .windows(2)
        .filter_map(|pair| match pair {
            [Word(name), Punct(b'!')] => Some(*name),
            _ => None,
        })
        .collect::<Vec<_>>();
    let mut child = 0;
    for item in items {
        let item = &tokens[item];
        if item_is_exact_cfg_test_v1(item) {
            continue;
        }
        let Some((attributes, attributes_end)) = leading_outer_attributes_v1(item) else {
            return false;
        };
        let default_would_mint_context = item.contains(&Word(CONTEXT))
            && attributes.iter().any(|attribute| {
                attribute.first() == Some(&Word("derive")) && attribute.contains(&Word("Default"))
            });
        if !production_outer_attributes_are_inert_v1(&attributes)
            || default_would_mint_context
            || has_production_expansion_surface_v1(item, &invoked)
        {
            return false;
        }
        let Some(module) = top_level_word_v1(item, "mod") else {
            continue;
        };
        let Some((expected_name, expected_path)) = expected.get(child).copied() else {
            return false;
        };
        if !item[attributes_end..module].is_empty()
            || item.get(module + 1) != Some(&Word(expected_name))
            || item.get(module + 2) != Some(&Punct(b';'))
            || module + 3 != item.len()
        {
            return false;
        }
        let mut matching_path = 0;
        for attribute in attributes {
            match attribute {
                [Word("path"), Punct(b'='), Literal(path)]
                    if string_literal_eq_v1(path, expected_path) =>
                {
                    matching_path += 1;
                }
                [Word("path"), ..] => return false,
                _ => {}
            }
        }
        if matching_path != 1 {
            return false;
        }
        child += 1;
    }
    child == expected.len()
}

fn descendant_module_has_no_context_mint_v1(source: &str) -> bool {
    use RustTokenV1::Word;
    const CONTEXT: &str = "ZkAmsPhase23RnsLinkContextV1";
    let Some(tokens) = rust_tokens_v1(source) else {
        return false;
    };
    let Some(items) = top_level_item_ranges_v1(&tokens) else {
        return false;
    };
    for item in items {
        let item = &tokens[item];
        if item_is_exact_cfg_test_v1(item) || !item.contains(&Word(CONTEXT)) {
            continue;
        }
        if let Some(implementation) = top_level_word_v1(item, "impl") {
            if !context_impl_has_no_production_mint_v1(item, implementation, CONTEXT) {
                return false;
            }
        } else if let Some(function) = top_level_word_v1(item, "fn") {
            if !function_context_surface_is_safe_v1(item, function, CONTEXT, false) {
                return false;
            }
        } else {
            return false;
        }
    }
    true
}

fn exact_pinned_context_descendant_tree_v1(sources: [&str; 8]) -> bool {
    let [
        root,
        cross_field,
        cross_field_joint_z,
        q_pcs,
        q_pcs_soundness,
        q_pcs_fri_rounds,
        q_pcs_canonical,
        q_pcs_verifier,
    ] = sources;
    const ROOT_CHILDREN: &[(&str, &str)] = &[
        ("cross_field_v2", "phase23_rns_link_cross_field_v2.rs"),
        ("q_pcs", "phase23_rns_link_q_pcs.rs"),
    ];
    const CROSS_FIELD_CHILDREN: &[(&str, &str)] = &[(
        "joint_z_binding_v3",
        "phase23_rns_link_cross_field_v2/joint_z_binding_v3.rs",
    )];
    const Q_PCS_CHILDREN: &[(&str, &str)] =
        &[("v2_soundness", "phase23_rns_link_q_pcs_v2_soundness.rs")];
    const SOUNDNESS_CHILDREN: &[(&str, &str)] = &[
        (
            "prover_fri_rounds_v2",
            "phase23_rns_link_q_pcs_v2_soundness/prover_fri_rounds_v2.rs",
        ),
        (
            "prover_canonical_proof_v2",
            "phase23_rns_link_q_pcs_v2_soundness/prover_canonical_proof_v2.rs",
        ),
        ("verifier", "phase23_rns_link_q_pcs_v2_verifier.rs"),
    ];
    #[rustfmt::skip]
    let descendants = [
        (
            cross_field,
            (
                42_350,
                [
                    0xe5, 0xce, 0xde, 0x64, 0x7f, 0x3c, 0x45, 0x48, 0x3f, 0xba, 0x60, 0x27,
                    0x2c, 0x3c, 0xfc, 0x94, 0x0b, 0x27, 0xca, 0x35, 0x36, 0xa8, 0xb3, 0x5c,
                    0x8a, 0xd2, 0x9e, 0x25, 0x8d, 0x6f, 0x76, 0x02,
                ],
            ),
            CROSS_FIELD_CHILDREN,
        ),
        (
            cross_field_joint_z,
            (
                9_152,
                [
                    0xc6, 0x44, 0x12, 0x04, 0x2b, 0x00, 0xf4, 0xee, 0xc4, 0x3c, 0xed, 0x4e,
                    0x5a, 0x11, 0xb9, 0x4b, 0xd2, 0x1d, 0x92, 0x22, 0x80, 0x42, 0x70, 0x10,
                    0xae, 0x8d, 0x25, 0x9c, 0x09, 0x82, 0xfc, 0x2c,
                ],
            ),
            &[][..],
        ),
        (
            q_pcs,
            (
                121_402,
                [
                    0x33, 0xe2, 0xb7, 0x24, 0xcc, 0x7e, 0x6c, 0x0c, 0xdb, 0xea, 0x98, 0xdc,
                    0x53, 0x07, 0xe6, 0x71, 0x8e, 0xdd, 0x01, 0x0a, 0x48, 0x48, 0x2d, 0xdf,
                    0x02, 0x6f, 0x39, 0xc0, 0xe4, 0x4d, 0xcc, 0xf0,
                ],
            ),
            Q_PCS_CHILDREN,
        ),
        (
            q_pcs_soundness,
            (
                51_990,
                [
                    0x11, 0x8e, 0x8a, 0xa6, 0xdf, 0xc6, 0xcf, 0x5e, 0x00, 0x9b, 0x81, 0x21,
                    0xbf, 0xa6, 0x97, 0xec, 0xef, 0xf0, 0x55, 0xe8, 0xe8, 0x00, 0xf3, 0xfe,
                    0x38, 0xd7, 0xaf, 0x29, 0x00, 0x1c, 0x51, 0xaf,
                ],
            ),
            SOUNDNESS_CHILDREN,
        ),
        (
            q_pcs_fri_rounds,
            (
                15_980,
                [
                    0xa7, 0xa4, 0x86, 0x14, 0x41, 0x84, 0x28, 0x09, 0x57, 0x1d, 0x61, 0xf0,
                    0x4b, 0xee, 0x4c, 0x20, 0x8f, 0x32, 0xcc, 0xab, 0xfd, 0x8f, 0x3d, 0x02,
                    0x65, 0x45, 0x45, 0xf3, 0x9e, 0xc9, 0x8d, 0x94,
                ],
            ),
            &[][..],
        ),
        (
            q_pcs_canonical,
            (
                14_380,
                [
                    0xb8, 0xe8, 0x4d, 0x22, 0x9f, 0x09, 0xd4, 0x4c, 0xf4, 0x30, 0xa0, 0xbd,
                    0xb3, 0x64, 0xf6, 0x7e, 0xd5, 0x6c, 0xf8, 0xab, 0xd9, 0x78, 0x8c, 0x3f,
                    0x39, 0xb3, 0xd6, 0xc3, 0xad, 0xfb, 0x9b, 0x9d,
                ],
            ),
            &[][..],
        ),
        (
            q_pcs_verifier,
            (
                23_434,
                [
                    0xaa, 0x69, 0x61, 0xfc, 0x23, 0x89, 0xb7, 0x9b, 0x87, 0xcf, 0xc8, 0x80,
                    0x47, 0x41, 0x4e, 0xbd, 0xff, 0xc6, 0x44, 0x40, 0x34, 0xb0, 0x9c, 0x01,
                    0x50, 0x2d, 0x04, 0x1d, 0x54, 0xb5, 0x4b, 0x93,
                ],
            ),
            &[][..],
        ),
    ];
    exact_production_child_modules_v1(root, ROOT_CHILDREN)
        && descendants.iter().all(|(source, pin, children)| {
            source_pin_v1(source) == *pin
                && exact_production_child_modules_v1(source, children)
                && descendant_module_has_no_context_mint_v1(source)
        })
}

fn exact_live_context_owner_authority_v1(source: &str) -> bool {
    const OWNER: &str = "ZkAmsPhase23RnsLinkContextOwnerV1";
    const CONTEXT: &str = "ZkAmsPhase23RnsLinkContextV1";
    const CONTEXT_AXIS_MAPPINGS: [&str; 9] = [
        "profile_digest: ContextAxisDigestV1(profile_digest)",
        "algorithm_manifest_digest: ContextAxisDigestV1(\n                immutable_algorithm_manifest_digest_v1()?\n            )",
        "network_context_digest: ContextAxisDigestV1(network_context_digest)",
        "statement_context_digest: ContextAxisDigestV1(proof_context.statement_digest)",
        "transcript_digest: ContextAxisDigestV1(terminal_context.transcript_digest)",
        "batch_digest: ContextAxisDigestV1(governed_batch.digest)",
        "roster_digest: ContextAxisDigestV1(terminal_context.roster_digest)",
        "direct_key_admission_digest: ContextAxisDigestV1(direct_key_admission_digest)",
        "canonical_map_set_digest: ContextAxisDigestV1(canonical_map_set_digest)",
    ];
    const RETURN_BINDINGS: [&str; 9] = [
        "profile_digest != terminal_context.profile_digest",
        "roster_digest != terminal_context.roster_digest",
        "epoch != terminal_context.epoch",
        "transcript_digest != terminal_context.transcript_digest",
        "batch_id != terminal_context.batch_id",
        "ordered_batch_input_digest != terminal_context.ordered_batch_input_digest",
        "fold_count != governed_fold_count",
        "collective_public_key_digest != direct_collective_public_key_digest",
        "key_material_digest != direct_key_material_digest",
    ];
    let Some(context_frame) = source.find("context_frame(proof_context)") else {
        return false;
    };
    let Some(network_hash) =
        source.find("let network_context_digest = keccak256(&generic_context_frame);")
    else {
        return false;
    };
    let Some(batch_validation) = source.find("terminal_composition_context_frame(") else {
        return false;
    };
    let Some(direct_validation) =
        source.find("direct_key_admission.validated_phase23_context_axes_v1(")
    else {
        return false;
    };
    let Some(context_mint) = source.find("let context = ZkAmsPhase23RnsLinkContextV1 {") else {
        return false;
    };
    let Some(context_return) =
        source.find("pub(in super::super) fn into_context_for_materialization_v1(")
    else {
        return false;
    };
    let Some(test_context_impl) = source.find("#[cfg(test)]\nimpl ZkAmsPhase23RnsLinkContextV1")
    else {
        return false;
    };
    let Some(test_context_constructor) = source.find("pub(in super::super) fn new(") else {
        return false;
    };
    let production_context_mint = &source[context_mint..context_return];
    context_frame < network_hash
        && network_hash < batch_validation
        && batch_validation < direct_validation
        && direct_validation < context_mint
        && context_mint < context_return
        && context_return < test_context_impl
        && test_context_impl < test_context_constructor
        && source
            .matches(&format!("pub(in super::super) struct {OWNER}"))
            .count()
            == 1
        && source
            .matches("pub(in super::super) fn from_native_sources_v1(")
            .count()
            == 1
        && source
            .matches("pub(in super::super) fn into_context_for_materialization_v1(")
            .count()
            == 1
        && source.matches("pub(in super::super) fn new(").count() == 1
        && source.matches("fn new(").count() == 1
        && source.matches("Ok(Self {").count() == 1
        && source
            .matches("let context = ZkAmsPhase23RnsLinkContextV1 {")
            .count()
            == 1
        && CONTEXT_AXIS_MAPPINGS
            .iter()
            .all(|mapping| production_context_mint.matches(*mapping).count() == 1)
        && source.contains("terminal_context.profile_digest != profile_digest")
        && source.contains(
            "terminal_context.nifs_verifier_digest != zk_ams_phase3_nifs_verifier_digest_v1()?",
        )
        && source.contains("governed_batch.context_digest != terminal_context.digest")
        && source.contains(
            "direct_key_admission.validated_phase23_context_axes_v1(\n            terminal_context.profile_digest,\n            terminal_context.roster_digest,\n            terminal_context.epoch,\n            terminal_context.transcript_digest,\n        )?",
        )
        && RETURN_BINDINGS
            .iter()
            .all(|binding| source.matches(*binding).count() == 1)
        && source.contains("context.validated_release_binding_digests_v1()?;")
        && !source.contains(&format!("impl Clone for {OWNER}"))
        && !source.contains(&format!("impl Copy for {OWNER}"))
        && !source.contains(&format!("impl Default for {OWNER}"))
        && !source.contains(&format!("impl Serialize for {OWNER}"))
        && !source.contains(&format!("impl Deserialize for {OWNER}"))
        && !source.contains(&format!("impl Encode for {OWNER}"))
        && !source.contains(&format!("impl Decode for {OWNER}"))
        && !source.contains(&format!("impl NoritoSerialize for {OWNER}"))
        && !source.contains(&format!("impl NoritoDeserialize for {OWNER}"))
        && !source.contains(&format!("impl Deref for {OWNER}"))
        && !source.contains(&format!("impl AsRef<{CONTEXT}> for {OWNER}"))
        && !source.contains(&format!("impl Borrow<{CONTEXT}> for {OWNER}"))
        && !source.contains("fn context(&self)")
        && !source.contains("fn context_mut(&mut self)")
        && !source.contains("fn as_context(&self)")
        && !source.contains("fn into_context(self)")
        && !source.contains("pub fn from_native_sources_v1")
        && !source.contains("pub fn into_context_for_materialization_v1")
        && source.contains(&format!("context: {CONTEXT},"))
}

fn exact_live_context_owner_consumer_v1(source: &str) -> bool {
    const CONTEXT_RETURN_CALL: &str = "let context = context_owner.into_context_for_materialization_v1(\n        profile_digest,\n        roster_digest,\n        authority.epoch(),\n        transcript_digest,\n        batch_id,\n        ordered_batch_input_digest,\n        fold_count,\n        authority.key_digest(),\n        authority.key_material_digest(),\n    )?;";
    let Some(consumer) = source.find("fn materialize_encrypt_and_publish_phase23_source_v1") else {
        return false;
    };
    let Some(owner_parameter) = source[consumer..]
        .find("context_owner: ZkAmsPhase23RnsLinkContextOwnerV1,")
        .map(|offset| consumer + offset)
    else {
        return false;
    };
    let Some(context_return) = source[consumer..]
        .find("let context = context_owner.into_context_for_materialization_v1(")
        .map(|offset| consumer + offset)
    else {
        return false;
    };
    let Some(source_begin) = source[consumer..]
        .find("ZkAmsPhase23RnsLinkExternalSourceAssemblyV1::begin_v1(context, directory)?")
        .map(|offset| consumer + offset)
    else {
        return false;
    };
    consumer < owner_parameter
        && owner_parameter < context_return
        && context_return < source_begin
        && source
            .matches("context_owner: ZkAmsPhase23RnsLinkContextOwnerV1,")
            .count()
            == 1
        && source
            .matches("context_owner.into_context_for_materialization_v1(")
            .count()
            == 1
        && source.matches(CONTEXT_RETURN_CALL).count() == 1
        && source.contains("authority.validate_release_v1()?;")
        && source.contains("authority.transcript_digest() != transcript_digest")
        && !source.contains("Phase23ContextCorrespondenceSealV1")
        && !source.contains("context: ZkAmsPhase23RnsLinkContextV1,")
}

fn exact_test_only_context_constructor_v1(source: &str) -> bool {
    use RustTokenV1::{Number, Punct, Word};
    const CONTEXT: &str = "ZkAmsPhase23RnsLinkContextV1";
    const NAMES: [&str; 7] = [
        "network_context_digest",
        "statement_context_digest",
        "transcript_digest",
        "batch_digest",
        "roster_digest",
        "direct_key_admission_digest",
        "canonical_map_set_digest",
    ];
    let Some(tokens) = rust_tokens_v1(source) else {
        return false;
    };
    let mut level = 0;
    let mut impl_body = None;
    for index in 0..tokens.len() {
        if level == 0 && tokens.get(index) == Some(&Word("impl")) {
            let (mut cursor, mut angles) = (index + 1, 0_usize);
            let open = loop {
                match tokens.get(cursor) {
                    Some(Punct(b'<')) => {
                        let Some(next) = angles.checked_add(1) else {
                            return false;
                        };
                        angles = next;
                    }
                    Some(Punct(b'>')) if angles != 0 => angles -= 1,
                    Some(Punct(open @ (b'(' | b'['))) => {
                        let close = if *open == b'(' { b')' } else { b']' };
                        let Some(end) = matching_pair_v1(&tokens, cursor, *open, close) else {
                            return false;
                        };
                        cursor = end;
                    }
                    Some(Punct(b'{')) if angles != 0 => {
                        let Some(end) = matching_pair_v1(&tokens, cursor, b'{', b'}') else {
                            return false;
                        };
                        cursor = end;
                    }
                    Some(Punct(b'{')) => break cursor,
                    Some(Punct(b';' | b'}')) | None => return false,
                    _ => {}
                }
                cursor += 1;
            };
            if tokens[index + 1..open].contains(&Word(CONTEXT)) {
                if impl_body.is_some() || tokens[index..open] != [Word("impl"), Word(CONTEXT)] {
                    return false;
                }
                let Some(end) = matching_pair_v1(&tokens, open, b'{', b'}') else {
                    return false;
                };
                impl_body = Some(open + 1..end);
            }
        }
        match tokens[index] {
            Punct(b'(' | b'[' | b'{') => level += 1,
            Punct(b')' | b']' | b'}') => level -= 1,
            _ => {}
        }
    }
    let Some(body) = impl_body.map(|range| &tokens[range]) else {
        return false;
    };
    let (mut depth, mut item_start, mut found) = (0, 0, false);
    for index in 0..body.len() {
        if depth == 0 && body.get(index) == Some(&Word("fn")) {
            let Some(Word(name)) = body.get(index + 1) else {
                return false;
            };
            let Some((_, _, returns, _, function_body)) = associated_function_parts_v1(body, index)
            else {
                return false;
            };
            let returns_context = body[returns]
                .iter()
                .any(|token| matches!(token, Word("Self")) || *token == Word(CONTEXT));
            if *name == "new" {
                if found || index < 4 {
                    return false;
                }
                found = true;
                let prefix = &body[item_start..index];
                if !prefix.ends_with(&[Word("pub"), Punct(b'('), Word("super"), Punct(b')')]) {
                    return false;
                }
                let Some(attributes) = outer_attributes_v1(&prefix[..prefix.len() - 4]) else {
                    return false;
                };
                let cfg_test = [Word("cfg"), Punct(b'('), Word("test"), Punct(b')')];
                if attributes
                    .iter()
                    .copied()
                    .filter(|body| *body == cfg_test.as_slice())
                    .count()
                    != 1
                    || attributes.iter().copied().any(|body| {
                        matches!(body.first(), Some(Word("cfg"))) && body != cfg_test.as_slice()
                    })
                {
                    return false;
                }
                let mut cursor = index + 2;
                if body.get(cursor) != Some(&Punct(b'(')) {
                    return false;
                }
                cursor += 1;
                for parameter_name in NAMES {
                    let parameter = [
                        Word(parameter_name),
                        Punct(b':'),
                        Punct(b'['),
                        Word("u8"),
                        Punct(b';'),
                        Number("32"),
                        Punct(b']'),
                        Punct(b','),
                    ];
                    if body.get(cursor..cursor + parameter.len()) != Some(parameter.as_slice()) {
                        return false;
                    }
                    cursor += parameter.len();
                }
                let suffix = [
                    Punct(b')'),
                    Punct(b'-'),
                    Punct(b'>'),
                    Word("Result"),
                    Punct(b'<'),
                    Word("Self"),
                    Punct(b','),
                    Word("ZkAmsMkheErrorV1"),
                    Punct(b'>'),
                    Punct(b'{'),
                ];
                let ok_self = [Word("Ok"), Punct(b'('), Word("Self"), Punct(b'{')];
                let ok_self_count = body[function_body.clone()]
                    .windows(ok_self.len())
                    .filter(|window| *window == ok_self.as_slice())
                    .count();
                if body.get(cursor..cursor + suffix.len()) != Some(suffix.as_slice())
                    || function_body.start != cursor + suffix.len()
                    || !returns_context
                    || direct_context_mints_v1(&body[function_body.clone()], CONTEXT) != 1
                    || ok_self_count != 1
                {
                    return false;
                }
            } else if returns_context || direct_context_mints_v1(&body[function_body], CONTEXT) != 0
            {
                return false;
            }
        }
        if depth == 0 && body.get(index) == Some(&Word("const")) {
            let (mut cursor, mut local_depth) = (index + 1, 0_usize);
            let mut function_modifier = false;
            while cursor < body.len() {
                match body[cursor] {
                    Word("fn") if local_depth == 0 => {
                        function_modifier = true;
                        break;
                    }
                    Punct(b';' | b'}') if local_depth == 0 => break,
                    Punct(b'(' | b'[' | b'{') => local_depth += 1,
                    Punct(b')' | b']' | b'}') => local_depth -= 1,
                    _ => {}
                }
                cursor += 1;
            }
            if !function_modifier {
                return false;
            }
        }
        if depth == 0
            && (matches!(body.get(index), Some(Word("type" | "static")))
                || body.get(index) == Some(&Punct(b'!')))
        {
            return false;
        }
        match body[index] {
            Punct(b'(' | b'[' | b'{') => depth += 1,
            Punct(b')' | b']' | b'}') => {
                depth -= 1;
                if depth == 0 && body[index] == Punct(b'}') {
                    item_start = index + 1;
                }
            }
            Punct(b';') if depth == 0 => item_start = index + 1,
            _ => {}
        }
    }
    found
        && (whole_module_context_mint_inventory_v1(source)
            || exact_live_context_owner_authority_v1(source))
}

fn exact_test_only_correspondence_seal_v1(source: &str) -> bool {
    use RustTokenV1::{Punct, Word};
    const SEAL: &str = "Phase23ContextCorrespondenceSealV1";
    const CONSUMER: &str = "materialize_encrypt_and_publish_phase23_source_v1";
    let Some(tokens) = rust_tokens_v1(source) else {
        return false;
    };
    let expected_body = [
        Punct(b'#'),
        Punct(b'['),
        Word("cfg"),
        Punct(b'('),
        Word("test"),
        Punct(b')'),
        Punct(b']'),
        Word("TestOnly"),
        Punct(b','),
    ];
    let (mut depth, mut item_start, mut enums, mut consumers) = (0, 0, 0, 0);
    for index in 0..tokens.len() {
        if tokens.get(index) == Some(&Word("enum"))
            && tokens.get(index + 1) == Some(&Word(SEAL))
            && tokens.get(index + 2) == Some(&Punct(b'{'))
        {
            enums += 1;
            let Some(end) = matching_pair_v1(&tokens, index + 2, b'{', b'}') else {
                return false;
            };
            if depth != 0
                || !outer_attributes_v1(&tokens[item_start..index])
                    .is_some_and(|attrs| attrs.is_empty())
                || tokens[index + 3..end] != expected_body
            {
                return false;
            }
        }
        if tokens.get(index) == Some(&Word("fn")) && tokens.get(index + 1) == Some(&Word(CONSUMER))
        {
            consumers += 1;
            if depth != 0 || outer_attributes_v1(&tokens[item_start..index]).is_none() {
                return false;
            }
            let Some(open) = tokens[index + 2..]
                .iter()
                .position(|token| *token == Punct(b'('))
                .map(|relative| index + 2 + relative)
            else {
                return false;
            };
            let first = [
                Word("_correspondence"),
                Punct(b':'),
                Word(SEAL),
                Punct(b','),
            ];
            if tokens.get(open + 1..open + 1 + first.len()) != Some(first.as_slice()) {
                return false;
            }
        }
        match tokens[index] {
            Punct(b'(' | b'[' | b'{') => depth += 1,
            Punct(b')' | b']' | b'}') => {
                depth -= 1;
                if depth == 0 && tokens[index] == Punct(b'}') {
                    item_start = index + 1;
                }
            }
            Punct(b';') if depth == 0 => item_start = index + 1,
            _ => {}
        }
    }
    enums == 1
        && consumers == 1
        && tokens.iter().filter(|token| **token == Word(SEAL)).count() == 2
        && tokens
            .iter()
            .filter(|token| **token == Word(CONSUMER))
            .count()
            == 1
}

fn test_digest_axes_v1() -> Phase23BundleDigestAxesV1 {
    Phase23BundleDigestAxesV1 {
        profile_digest: [1; 32],
        roster_digest: [2; 32],
        materialized_transcript_digest: [3; 32],
        batch_id: [4; 32],
        ordered_batch_input_digest: [5; 32],
        fold_count: 6,
        shape: ZkAmsPhase23AccumulatorShapeV1::new(
            PHASE23_X_VALUES_V1,
            PHASE23_U_AND_E_VALUES_V1,
            PHASE23_RE_VALUES_V1,
            PHASE23_W_VALUES_V1,
            PHASE23_RW_VALUES_V1,
        )
        .unwrap(),
        materialized_digest: [7; 32],
        key_digest: [8; 32],
        key_authority_digest: [9; 32],
        key_epoch: 10,
        source_receipt_digest: [11; 32],
        native_bgv_opening_receipt_set_digest: [12; 32],
        public_artifact_manifest_bound: true,
    }
}
fn test_manifest_digests_v1() -> [[u8; 32]; PHASE23_RECORD_COUNT_V1] {
    core::array::from_fn(|ordinal| [u8::try_from(ordinal + 1).unwrap(); 32])
}
#[test]
fn exact_record_schedule_is_x_u_e_re_w_rw() {
    let positions = (0..PHASE23_RECORD_COUNT_V1)
        .map(|ordinal| phase23_record_position_v1(u16::try_from(ordinal).unwrap()).unwrap())
        .collect::<Vec<_>>();
    let expected = [
        (ZkAmsPhase23RnsLinkFamilyV1::X, 1_usize),
        (ZkAmsPhase23RnsLinkFamilyV1::U, 16),
        (ZkAmsPhase23RnsLinkFamilyV1::E, 16),
        (ZkAmsPhase23RnsLinkFamilyV1::RE, 1),
        (ZkAmsPhase23RnsLinkFamilyV1::W, 8),
        (ZkAmsPhase23RnsLinkFamilyV1::RW, 1),
    ];
    let mut offset = 0;
    for (family, count) in expected {
        for (chunk_index, position) in positions[offset..offset + count].iter().enumerate() {
            assert_eq!(position.family, family);
            assert_eq!(usize::from(position.chunk_index), chunk_index);
            assert_eq!(usize::from(position.family_chunk_count), count);
            assert_ne!(position.layout_v1().unwrap().digest, [0; 32]);
            assert!(position.used_slots_v1().unwrap() > 0);
        }
        offset += count;
    }
    assert_eq!(offset, PHASE23_RECORD_COUNT_V1);
    assert_eq!(positions[0].used_slots_v1().unwrap(), 89);
    assert_eq!(positions[33].used_slots_v1().unwrap(), 1_024);
    assert_eq!(positions[42].used_slots_v1().unwrap(), 512);
    assert!(phase23_record_position_v1(43).is_err());
}
#[test]
fn hostile_schedule_coordinates_fail_before_the_encryption_core() {
    let position = phase23_record_position_v1(5).unwrap();
    let layout = position.layout_v1().unwrap();
    let mut packed = ZkAmsT256PackedPlaintextV1 {
        version: 1,
        profile_digest: layout.profile_digest,
        layout_digest: layout.digest,
        chunk_index: u32::from(position.chunk_index),
        used_slots: position.used_slots_v1().unwrap(),
        coefficients: Vec::new(),
        digest: [1; 32],
    };
    assert_eq!(
        require_expected_packed_coordinate_v1(position, layout, &packed).unwrap(),
        layout.slots_per_chunk
    );
    packed.chunk_index += 1;
    assert!(require_expected_packed_coordinate_v1(position, layout, &packed).is_err());
    packed.chunk_index = u32::from(position.chunk_index);
    packed.used_slots -= 1;
    assert!(require_expected_packed_coordinate_v1(position, layout, &packed).is_err());
    packed.used_slots = position.used_slots_v1().unwrap();
    packed.layout_digest[0] ^= 1;
    assert!(require_expected_packed_coordinate_v1(position, layout, &packed).is_err());
    packed.layout_digest = layout.digest;
    let foreign_layout = phase23_record_position_v1(0).unwrap().layout_v1().unwrap();
    assert!(require_expected_packed_coordinate_v1(position, foreign_layout, &packed).is_err());
}
#[test]
fn named_peak_includes_the_preallocated_secret_chunk_pool() {
    assert_eq!(PHASE23_SECRET_CHUNK_POOL_PAYLOAD_BYTES_V1, 7_340_064);
    const {
        assert!(PHASE23_SECRET_CHUNK_POOL_METADATA_BYTES_V1 > 0);
        assert!(PHASE23_NAMED_HEAP_PEAK_BYTES_V1 < 160 * 1_048_576);
        assert!(PHASE23_NATIVE_BGV_OPENING_RECEIPT_OWNER_BYTES_V1 > 0);
    };
    assert_eq!(PHASE23_ONE_PACKED_CHUNK_BYTES_V1, 4 * 1_048_576);
    assert_eq!(PHASE23_DECODER_WORKSPACE_BYTES_V1, 8 * 1_048_576);
    assert_eq!(PHASE23_COMPACT_MANIFEST_OWNER_BYTES_V1, 4_718_592);
}
#[test]
fn bundle_digest_has_an_independent_exact_kat_and_changes_every_bound_axis() {
    let axes = test_digest_axes_v1();
    let manifests = test_manifest_digests_v1();
    let digest = phase23_bundle_digest_from_frames_v1(axes, &manifests).unwrap();
    assert_eq!(
        hex::encode(digest),
        "ce852dbf2d39f23dfe59eb559fa8ebf166a7f514e42b15761fa4251713528c3a"
    );
    let mut changed_axes = axes;
    changed_axes.source_receipt_digest[0] ^= 1;
    assert_ne!(
        phase23_bundle_digest_from_frames_v1(changed_axes, &manifests).unwrap(),
        digest
    );
    let mut changed_native_opening_receipts = axes;
    changed_native_opening_receipts.native_bgv_opening_receipt_set_digest[0] ^= 1;
    assert_ne!(
        phase23_bundle_digest_from_frames_v1(changed_native_opening_receipts, &manifests).unwrap(),
        digest
    );
    let mut changed_shape = axes;
    changed_shape.shape.x += 1;
    assert!(phase23_bundle_digest_from_frames_v1(changed_shape, &manifests).is_err());
    let mut changed_manifest = manifests;
    changed_manifest[17][0] ^= 1;
    assert_ne!(
        phase23_bundle_digest_from_frames_v1(axes, &changed_manifest).unwrap(),
        digest
    );
    let mut unbound = axes;
    unbound.public_artifact_manifest_bound = false;
    assert!(phase23_bundle_digest_from_frames_v1(unbound, &manifests).is_err());
}

#[test]
fn native_bgv_opening_receipts_are_retained_validated_and_bundle_bound() {
    let parent = include_str!("incremental_source.rs");
    let source = include_str!("incremental_source_phase23.rs");
    let owner = source
        .split("struct ZkAmsPhase23MaterializedEncryptedSourceOwnerV1")
        .nth(1)
        .expect("Phase23 owner")
        .split("struct Phase23MaterializeEncryptChunkStreamV1")
        .next()
        .expect("Phase23 owner boundary");
    assert!(
        owner.contains(
            "native_bgv_opening_receipts: Vec<VerifiedStreamingNativeBgvOpeningReceiptV1>"
        )
    );
    assert!(owner.contains("native_bgv_opening_receipt.validate_for_manifest_v1(manifest)?"));
    assert!(owner.contains("phase23_native_bgv_opening_receipt_set_digest_v1"));

    let stream = source
        .split("impl<I, R, K, P> Iterator for Phase23MaterializeEncryptChunkStreamV1")
        .nth(1)
        .expect("Phase23 stream")
        .split("fn materialize_encrypt_and_publish_phase23_source_v1")
        .next()
        .expect("Phase23 stream boundary");
    let split = stream.find("product.into_verified_parts_v1()").unwrap();
    let manifest_push = stream.find("self.manifests.push(manifest)").unwrap();
    let receipt_push = stream.find(".push(native_bgv_opening_receipt)").unwrap();
    assert!(
        parent.contains("fn into_verified_parts_v1(")
            && parent.contains("native_bgv_opening_receipt.validate_for_manifest_v1(&manifest)?;")
    );
    assert!(split < manifest_push);
    assert!(manifest_push < receipt_push);
    assert!(source.contains("hash.update(&axes.native_bgv_opening_receipt_set_digest);"));
}

#[test]
fn structural_gate_preserves_validation_entropy_source_and_output_order() {
    let parent = include_str!("incremental_source.rs");
    let source = include_str!("incremental_source_phase23.rs");
    let external = include_str!("../phase23_rns_link_external_source.rs");
    let packing = include_str!("../packing.rs");
    let core = parent
        .split("fn encrypt_zk_ams_mkhe_collective_packed_streaming_borrowed_with_prepublication_v1")
        .nth(1)
        .unwrap()
        .split("pub fn encrypt_zk_ams_mkhe_collective_packed_streaming_v1")
        .next()
        .unwrap();
    let validation = core
        .find("ValidatedT256PackedPlaintextV1::validate_for_release_limb_stream_v1")
        .unwrap();
    let pool_factory = core.find("prepare_before_entropy()?").unwrap();
    let prepared = core
        .find("PreparedStreamingCollectiveEncryptionV1::new_v1")
        .unwrap();
    let entropy = core.find("authenticated.activate_v1").unwrap();
    let source_callback = core.find("before_output_publication(").unwrap();
    let output = core.find("active.publish_all_v1").unwrap();
    assert!(validation < pool_factory);
    assert!(pool_factory < prepared);
    assert!(prepared < entropy);
    assert!(entropy < source_callback);
    assert!(source_callback < output);
    assert!(core.contains("F: FnOnce("));
    assert!(!core.contains("dyn Fn"));
    assert!(source.contains("try_reserve_exact(PHASE23_MAIN_BLOCKS_PER_RECORD_V1)"));
    assert!(source.contains("main.capacity() != PHASE23_MAIN_BLOCKS_PER_RECORD_V1"));
    assert!(source.contains("Phase23SecretRecordChunkPoolV1::try_new_exact_v1()?"));
    let coordinate_check = source
        .find("match require_expected_packed_coordinate_v1(position, layout, &packed)")
        .unwrap();
    let encryption_call = source
        .find("encrypt_zk_ams_mkhe_collective_packed_streaming_borrowed_with_prepublication_v1(")
        .unwrap();
    assert!(coordinate_check < encryption_call);
    let canonical = source
        .find("write_next_canonical_plaintext_block_v1")
        .unwrap();
    let ephemeral = source.find("write_next_ephemeral_block_v1").unwrap();
    let error_zero = source.find("write_next_error_zero_block_v1").unwrap();
    let error_one = source.find("write_next_error_one_block_v1").unwrap();
    let nonce = source.find("write_next_nonce_v1").unwrap();
    assert!(canonical < ephemeral && ephemeral < error_zero);
    assert!(error_zero < error_one && error_one < nonce);
    assert!(source.contains("Some(Ok(packed))"));
    assert!(source.contains("&packed,"));
    assert!(external.contains("let mut live = self\n            .live\n            .take()"));
    assert!(packing.contains("impl Drop for ZkAmsT256PackedPlaintextV1"));
}
#[test]
fn structural_gate_is_fail_closed_and_returns_one_move_only_owner_only_on_success() {
    let parent = include_str!("incremental_source.rs");
    let source = include_str!("incremental_source_phase23.rs");
    let external = include_str!("../phase23_rns_link_external_source.rs");
    let encrypted = include_str!("../phase23_encrypted.rs");
    let leaf = include_str!("../../../../../../iroha_crypto/src/confidential_spool.rs");
    assert!(source.contains("let next = self.chunks.next()?;"));
    assert!(source.contains(">= PHASE23_RECORD_COUNT_V1"));
    assert!(source.contains("Err(error) => return Some(Err(error))"));
    assert!(source.contains("source.finish_v1()?"));
    assert!(source.contains("owner.validate_v1()?;\n    Ok(owner)"));
    assert!(parent.contains("authority.failed = true;"));
    assert!(parent.contains("active.kernel.canonical_plaintext"));
    assert!(parent.contains("active.kernel.ephemeral.as_slice()"));
    assert!(parent.contains("active.kernel.error_zero.as_slice()"));
    assert!(parent.contains("active.kernel.error_one.as_slice()"));
    assert!(parent.contains("active.kernel.input_identity.encryption_nonce.as_bytes()"));
    assert!(source.contains("pool.persist_exact_record_v1("));
    assert!(source.contains("struct ZkAmsPhase23MaterializedEncryptedSourceOwnerV1<K, P>"));
    assert!(source.contains("source: ZkAmsPhase23RnsLinkExternalSourcePublicationV1"));
    assert!(source.contains("manifests: Vec<ZkAmsMkheStreamingCollectiveCiphertextV1>"));
    assert!(source.contains("public_artifact_manifest_bound: true"));
    assert!(
        !source.contains("impl<K, P> Clone for ZkAmsPhase23MaterializedEncryptedSourceOwnerV1")
    );
    assert!(!source.contains("Serialize"));
    assert!(!source.contains("Decode"));
    assert!(!source.contains("pub use"));
    assert!(external.contains("const PUBLIC_ARTIFACT_MANIFEST_BOUND_V1: bool = false;"));
    assert!(external.contains("const SOURCE_RELATION_POLYNOMIALS_CONSTRUCTED_V1: bool = false;"));
    assert!(external.contains("const RELEASE_COMPLETE_V1: bool = false;"));
    assert!(encrypted.contains("impl Drop for ZkAmsPhase23MaterializedAccumulatorsV1"));
    assert!(leaf.contains("impl Drop for ConfidentialSpoolChunkV1"));
    assert!(!source.contains("mem::forget"));
}
#[test]
fn module_graph_and_context_authority_use_exact_live_owner_and_remain_private() {
    let incremental = include_str!("incremental_source.rs");
    let collective = include_str!("../collective.rs");
    let mkhe = include_str!("../../mkhe.rs");
    let source = include_str!("incremental_source_phase23.rs");
    let rns_link = include_str!("../phase23_rns_link.rs");
    let context_authority = include_str!("../phase23_rns_link_context_authority_v1.rs");
    let cross_field = include_str!("../phase23_rns_link_cross_field_v2.rs");
    let cross_field_joint_z =
        include_str!("../phase23_rns_link_cross_field_v2/joint_z_binding_v3.rs");
    let q_pcs = include_str!("../phase23_rns_link_q_pcs.rs");
    let q_pcs_soundness = include_str!("../phase23_rns_link_q_pcs_v2_soundness.rs");
    let q_pcs_fri_rounds =
        include_str!("../phase23_rns_link_q_pcs_v2_soundness/prover_fri_rounds_v2.rs");
    let q_pcs_canonical =
        include_str!("../phase23_rns_link_q_pcs_v2_soundness/prover_canonical_proof_v2.rs");
    let q_pcs_verifier = include_str!("../phase23_rns_link_q_pcs_v2_verifier.rs");
    assert!(exact_private_path_module_v1(
        mkhe,
        "collective",
        "mkhe/collective.rs"
    ));
    assert!(exact_private_path_module_v1(
        collective,
        "incremental_source",
        "collective/incremental_source.rs"
    ));
    assert!(exact_private_path_module_v1(
        incremental,
        "incremental_source_phase23",
        "incremental_source_phase23.rs"
    ));
    assert!(exact_private_path_module_v1(
        mkhe,
        "global_lookup_statement_v1",
        "mkhe/global_lookup_statement_v1.rs"
    ));
    assert!(exact_private_path_module_v1(
        mkhe,
        "phase23_rns_link",
        "mkhe/phase23_rns_link.rs"
    ));
    assert!(exact_word_count_v1(
        incremental,
        "incremental_source_phase23",
        1
    ));
    assert!(exact_word_count_v1(mkhe, "global_lookup_statement_v1", 1));
    assert!(exact_word_count_v1(mkhe, "phase23_rns_link", 1));
    assert!(contains_no_words_v1(
        collective,
        &["incremental_source_phase23"]
    ));
    assert!(contains_no_words_v1(mkhe, &["incremental_source_phase23"]));
    for facade in [incremental, collective, rns_link, mkhe] {
        assert!(contains_no_words_v1(
            facade,
            &[
                "Phase23ContextCorrespondenceSealV1",
                "materialize_encrypt_and_publish_phase23_source_v1"
            ]
        ));
    }
    assert!(exact_live_context_owner_authority_v1(context_authority));
    assert!(exact_pinned_context_descendant_tree_v1([
        rns_link,
        cross_field,
        cross_field_joint_z,
        q_pcs,
        q_pcs_soundness,
        q_pcs_fri_rounds,
        q_pcs_canonical,
        q_pcs_verifier,
    ]));
    assert!(exact_live_context_owner_consumer_v1(source));
}

#[test]
fn live_context_owner_source_contract_rejects_axis_and_return_path_mutations() {
    const AUTHORITY: &str = include_str!("../phase23_rns_link_context_authority_v1.rs");
    const CONSUMER: &str = include_str!("incremental_source_phase23.rs");
    assert!(exact_live_context_owner_authority_v1(AUTHORITY));
    assert!(exact_live_context_owner_consumer_v1(CONSUMER));

    for mutated in [
        AUTHORITY.replacen("context_frame(proof_context)", "proof_context.chain_id", 1),
        AUTHORITY.replacen(
            "let network_context_digest = keccak256(&generic_context_frame);",
            "let network_context_digest = proof_context.genesis_hash;",
            1,
        ),
        AUTHORITY.replacen(
            "terminal_composition_context_frame(",
            "Vec::<u8>::with_capacity(",
            1,
        ),
        AUTHORITY.replacen(
            "governed_batch.context_digest != terminal_context.digest",
            "governed_batch.context_digest == terminal_context.digest",
            1,
        ),
        AUTHORITY.replacen(
            "direct_key_admission.validated_phase23_context_axes_v1(",
            "direct_key_admission.digest(",
            1,
        ),
        AUTHORITY.replacen(
            "statement_context_digest: ContextAxisDigestV1(proof_context.statement_digest)",
            "statement_context_digest: ContextAxisDigestV1(terminal_context.digest)",
            1,
        ),
        AUTHORITY.replacen(
            "batch_digest: ContextAxisDigestV1(governed_batch.digest)",
            "batch_digest: ContextAxisDigestV1(terminal_context.batch_id)",
            1,
        ),
        AUTHORITY.replacen(
            "profile_digest: ContextAxisDigestV1(profile_digest)",
            "profile_digest: ContextAxisDigestV1(terminal_context.digest)",
            1,
        ),
        AUTHORITY.replacen(
            "immutable_algorithm_manifest_digest_v1()?\n            )",
            "profile_digest\n            )",
            1,
        ),
        AUTHORITY.replacen(
            "network_context_digest: ContextAxisDigestV1(network_context_digest)",
            "network_context_digest: ContextAxisDigestV1(proof_context.genesis_hash)",
            1,
        ),
        AUTHORITY.replacen(
            "transcript_digest: ContextAxisDigestV1(terminal_context.transcript_digest)",
            "transcript_digest: ContextAxisDigestV1(terminal_context.digest)",
            1,
        ),
        AUTHORITY.replacen(
            "roster_digest: ContextAxisDigestV1(terminal_context.roster_digest)",
            "roster_digest: ContextAxisDigestV1(terminal_context.digest)",
            1,
        ),
        AUTHORITY.replacen(
            "direct_key_admission_digest: ContextAxisDigestV1(direct_key_admission_digest)",
            "direct_key_admission_digest: ContextAxisDigestV1(direct_key_material_digest)",
            1,
        ),
        AUTHORITY.replacen(
            "canonical_map_set_digest: ContextAxisDigestV1(canonical_map_set_digest)",
            "canonical_map_set_digest: ContextAxisDigestV1(profile_digest)",
            1,
        ),
        AUTHORITY.replacen(
            "terminal_context.profile_digest != profile_digest",
            "terminal_context.profile_digest == profile_digest",
            1,
        ),
        AUTHORITY.replacen(
            "terminal_context.nifs_verifier_digest != zk_ams_phase3_nifs_verifier_digest_v1()?",
            "terminal_context.nifs_verifier_digest == zk_ams_phase3_nifs_verifier_digest_v1()?",
            1,
        ),
        AUTHORITY.replacen(
            "terminal_context.profile_digest,\n            terminal_context.roster_digest,\n            terminal_context.epoch,\n            terminal_context.transcript_digest,",
            "[0; 32],\n            terminal_context.roster_digest,\n            terminal_context.epoch,\n            terminal_context.transcript_digest,",
            1,
        ),
        AUTHORITY.replacen(
            "terminal_context.profile_digest,\n            terminal_context.roster_digest,\n            terminal_context.epoch,\n            terminal_context.transcript_digest,",
            "terminal_context.profile_digest,\n            [0; 32],\n            terminal_context.epoch,\n            terminal_context.transcript_digest,",
            1,
        ),
        AUTHORITY.replacen(
            "terminal_context.profile_digest,\n            terminal_context.roster_digest,\n            terminal_context.epoch,\n            terminal_context.transcript_digest,",
            "terminal_context.profile_digest,\n            terminal_context.roster_digest,\n            0,\n            terminal_context.transcript_digest,",
            1,
        ),
        AUTHORITY.replacen(
            "terminal_context.profile_digest,\n            terminal_context.roster_digest,\n            terminal_context.epoch,\n            terminal_context.transcript_digest,",
            "terminal_context.profile_digest,\n            terminal_context.roster_digest,\n            terminal_context.epoch,\n            [0; 32],",
            1,
        ),
        AUTHORITY.replacen(
            "collective_public_key_digest != direct_collective_public_key_digest",
            "collective_public_key_digest == direct_collective_public_key_digest",
            1,
        ),
        AUTHORITY.replacen(
            "key_material_digest != direct_key_material_digest",
            "key_material_digest == direct_key_material_digest",
            1,
        ),
        AUTHORITY.replacen(
            "profile_digest != terminal_context.profile_digest",
            "profile_digest == terminal_context.profile_digest",
            1,
        ),
        AUTHORITY.replacen(
            "roster_digest != terminal_context.roster_digest",
            "roster_digest == terminal_context.roster_digest",
            1,
        ),
        AUTHORITY.replacen(
            "epoch != terminal_context.epoch",
            "epoch == terminal_context.epoch",
            1,
        ),
        AUTHORITY.replacen(
            "transcript_digest != terminal_context.transcript_digest",
            "transcript_digest == terminal_context.transcript_digest",
            1,
        ),
        AUTHORITY.replacen(
            "batch_id != terminal_context.batch_id",
            "batch_id == terminal_context.batch_id",
            1,
        ),
        AUTHORITY.replacen(
            "ordered_batch_input_digest != terminal_context.ordered_batch_input_digest",
            "ordered_batch_input_digest == terminal_context.ordered_batch_input_digest",
            1,
        ),
        AUTHORITY.replacen(
            "fold_count != governed_fold_count",
            "fold_count == governed_fold_count",
            1,
        ),
        AUTHORITY.replacen(
            "pub(in super::super) fn from_native_sources_v1(",
            "pub(crate) fn from_native_sources_v1(",
            1,
        ),
        AUTHORITY.replacen(
            "#[cfg(test)]\nimpl ZkAmsPhase23RnsLinkContextV1",
            "impl ZkAmsPhase23RnsLinkContextV1",
            1,
        ),
        format!(
            "{AUTHORITY}\nimpl ZkAmsPhase23RnsLinkContextOwnerV1 {{ fn context(&self) -> &ZkAmsPhase23RnsLinkContextV1 {{ &self.context }} }}"
        ),
        format!("{AUTHORITY}\nimpl Clone for ZkAmsPhase23RnsLinkContextOwnerV1 {{}}"),
        format!("{AUTHORITY}\nimpl Decode for ZkAmsPhase23RnsLinkContextOwnerV1 {{}}"),
    ] {
        assert!(!exact_live_context_owner_authority_v1(&mutated));
    }

    for mutated in [
        CONSUMER.replacen(
            "context_owner: ZkAmsPhase23RnsLinkContextOwnerV1,",
            "context: ZkAmsPhase23RnsLinkContextV1,",
            1,
        ),
        CONSUMER.replacen("authority.validate_release_v1()?;", "", 1),
        CONSUMER.replacen(
            "authority.transcript_digest() != transcript_digest",
            "authority.transcript_digest() == transcript_digest",
            1,
        ),
        CONSUMER.replacen(
            "        authority.epoch(),\n        transcript_digest,",
            "        0,\n        transcript_digest,",
            1,
        ),
        CONSUMER.replacen(
            "        authority.key_digest(),\n        authority.key_material_digest(),",
            "        [0; 32],\n        authority.key_material_digest(),",
            1,
        ),
        CONSUMER.replacen("authority.key_material_digest(),", "[0; 32],", 1),
        format!("{CONSUMER}\nenum Phase23ContextCorrespondenceSealV1 {{ Escape }}"),
    ] {
        assert!(!exact_live_context_owner_consumer_v1(&mutated));
    }
}

#[test]
fn source_authority_oracle_rejects_lexical_visibility_and_mint_decoys() {
    const DECOYS: &str = r####"
// mod incremental_source_phase23;
/* outer /* pub(crate) mod incremental_source_phase23; impl ZkAmsPhase23RnsLinkContextV1 {} */ enum Phase23ContextCorrespondenceSealV1 { Production } */
const COOKED: &str = "mod incremental_source_phase23; { }";
const BYTE: &[u8] = b"pub mod incremental_source_phase23; { }";
const C_STRING: &CStr = c"pub(super) mod incremental_source_phase23; { }";
const RAW: &str = r#"pub(in crate) mod incremental_source_phase23; { }"#;
const BYTE_RAW: &[u8] = br##"mod incremental_source_phase23; { }"##;
const C_RAW: &CStr = cr##"mod incremental_source_phase23; { }"##;
const AUTHORITY: &str = "impl ZkAmsPhase23RnsLinkContextV1 { pub(super) fn new() {} }";
const SEAL_NOTE: &[u8] = b"enum Phase23ContextCorrespondenceSealV1 { Production }";
const CONSUMER_NOTE: &CStr = c"pub fn materialize_encrypt_and_publish_phase23_source_v1()";
const CLOSE: char = '}'; const OPEN: char = '{';
const CLOSE_BYTE: u8 = b'}'; const OPEN_BYTE: u8 = b'{';
const QUOTE: char = '\''; const QUOTE_BYTE: u8 = b'\'';
const SLASH: char = '\\'; const SLASH_BYTE: u8 = b'\\';
const HEX: char = '\x7b'; const HEX_BYTE: u8 = b'\x7d'; const UNICODE: char = '\u{7d}';
fn lifetimes<'a>(_: &'a str) { 'label: loop { break 'label; } }
"####;
    let private_module = format!(
        "{DECOYS}\n#[allow(dead_code)]\n#[doc = \"mod incremental_source_phase23;\"]\n#[path = r#\"incremental_source_phase23.rs\"#]\nmod\nincremental_source_phase23\n;"
    );
    assert!(exact_private_path_module_v1(
        &private_module,
        "incremental_source_phase23",
        "incremental_source_phase23.rs"
    ));
    for visible in [
        "pub mod incremental_source_phase23;",
        "pub(crate) mod incremental_source_phase23;",
        "pub ( crate ) mod incremental_source_phase23;",
        "pub(super)\nmod incremental_source_phase23;",
        "pub ( super )\nmod incremental_source_phase23;",
        "pub(in crate) mod incremental_source_phase23;",
        "pub ( in crate::parent ) mod incremental_source_phase23;",
        "pub /* split visibility */ (crate) mod incremental_source_phase23;",
        "fn nested<'a>() { mod incremental_source_phase23; 'label: loop { break 'label; } }",
        "mod wrapper { #[path = \"incremental_source_phase23.rs\"] mod incremental_source_phase23; }",
        "#[cfg(test)] #[path = \"incremental_source_phase23.rs\"] mod incremental_source_phase23;",
    ] {
        assert!(!exact_private_path_module_v1(
            &format!("{DECOYS}\n#[path = \"incremental_source_phase23.rs\"]\n{visible}"),
            "incremental_source_phase23",
            "incremental_source_phase23.rs"
        ));
    }

    const BENIGN_CHILD_PARENT: &str = r#"
#[allow(dead_code)]
#[path = "pinned_child.rs"]
mod pinned_child;
#[cfg(test)]
mod tests {
    include!("test_only_expansion.rs");
    mod nested_test_child;
}
#[cfg(test)]
include!("test_only_item.rs");
"#;
    const PINNED_CHILD: [(&str, &str); 1] = [("pinned_child", "pinned_child.rs")];
    assert!(exact_production_child_modules_v1(
        BENIGN_CHILD_PARENT,
        &PINNED_CHILD
    ));
    for expansion_escape in [
        "include!(\"context_mint.rs\");",
        "fn load_mint() { include!(\"context_mint.rs\"); }",
        "use std::include as inc; fn hidden() { inc!(\"context_mint_expr.rs\"); }",
        "fn hidden() { use std::include as vec; vec![\"context_mint_expr.rs\"]; }",
        "use std::{include as inc}; fn hidden() { self::inc!(\"context_mint_expr.rs\"); }",
        "pub use std::include as inc; fn hidden() { inc!(\"context_mint_expr.rs\"); }",
        "fn hidden() { let _ = || include!(\"context_mint_expr.rs\"); }",
        "fn hidden() { match () { () => include!(\"context_mint_expr.rs\") } }",
        "const HIDDEN: () = { include!(\"context_mint_expr.rs\"); };",
        "expand_context_mint!();",
        "macro_rules! expand_context_mint { () => { mod escaped; }; }",
        "fn hidden() { macro_rules! local { () => {} } local!(); }",
        "mod inline_escape { fn mint() {} }",
        "#[path = \"context_mint.rs\"] mod context_mint;",
    ] {
        assert!(!exact_production_child_modules_v1(
            &format!("{BENIGN_CHILD_PARENT}\n{expansion_escape}"),
            &PINNED_CHILD
        ));
    }
    const BENIGN_DESCENDANT: &str = r#"
use crate::{Point as P, Scalar as S};
const SHAPE_OK: () = { assert!(true); };
fn ordinary_macros() { let _ = vec![0_u8]; let _ = write!(sink, "{value}"); }
pub(super) fn inspect(
    context: &ZkAmsPhase23RnsLinkContextV1,
) -> u8 {
    context.profile_digest[0]
}
#[cfg(test)]
mod tests {
    fn mint() -> ZkAmsPhase23RnsLinkContextV1 {
        ZkAmsPhase23RnsLinkContextV1 { profile_digest: [0; 32] }
    }
}
#[cfg(test)]
std::thread_local! { static TEST_ONLY: core::cell::Cell<u8> = const { core::cell::Cell::new(0) }; }
"#;
    assert!(exact_production_child_modules_v1(BENIGN_DESCENDANT, &[]));
    assert!(descendant_module_has_no_context_mint_v1(BENIGN_DESCENDANT));
    assert!(!exact_production_child_modules_v1(
        "#[derive(Default)] pub(super) struct ZkAmsPhase23RnsLinkContextV1 { field: u8 }",
        &[]
    ));
    const OUT_OF_LINE_CHILD_MINT: &str = r#"
pub(super) fn mint() -> ZkAmsPhase23RnsLinkContextV1 {
    super::ZkAmsPhase23RnsLinkContextV1 { profile_digest: [0; 32] }
}
"#;
    assert!(!descendant_module_has_no_context_mint_v1(
        OUT_OF_LINE_CHILD_MINT
    ));
    let mutated_child = format!("{BENIGN_DESCENDANT}\n{OUT_OF_LINE_CHILD_MINT}");
    assert!(exact_production_child_modules_v1(&mutated_child, &[]));
    assert!(!descendant_module_has_no_context_mint_v1(&mutated_child));
    const NESTED_DESCENDANT: &str = r#"
#[path = "grandchild.rs"]
mod grandchild;
"#;
    assert!(exact_production_child_modules_v1(
        NESTED_DESCENDANT,
        &[("grandchild", "grandchild.rs")]
    ));
    assert!(!exact_production_child_modules_v1(NESTED_DESCENDANT, &[]));
    for descendant_expansion in ["include!(\"mint.rs\");", "expand_context_mint!();"] {
        assert!(!exact_production_child_modules_v1(
            descendant_expansion,
            &[]
        ));
    }
    assert!(!descendant_module_has_no_context_mint_v1(
        r#"type MintAlias = ZkAmsPhase23RnsLinkContextV1;"#
    ));

    const CONSTRUCTOR: &str = r#"
impl ZkAmsPhase23RnsLinkContextV1 {
    #[cfg ( test )]
    #[allow(clippy::too_many_arguments)]
    pub
    (
        super
    )
    fn new(
        network_context_digest: [u8; 32],
        statement_context_digest: [u8; 32],
        transcript_digest: [u8; 32],
        batch_digest: [u8; 32],
        roster_digest: [u8; 32],
        direct_key_admission_digest: [u8; 32],
        canonical_map_set_digest: [u8; 32],
    ) -> Result<Self, ZkAmsMkheErrorV1> { Ok(Self { profile_digest: [0; 32] }) }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct ZkAmsPhase23RnsLinkContextV1 {
    profile_digest: [u8; 32],
}
"#;
    assert!(exact_test_only_context_constructor_v1(&format!(
        "{DECOYS}{CONSTRUCTOR}"
    )));
    assert!(!exact_test_only_context_constructor_v1(
        &CONSTRUCTOR.replacen(
            "#[cfg ( test )]",
            "#[cfg(any(test, feature = \"production_escape\"))]",
            1
        )
    ));
    assert!(!exact_test_only_context_constructor_v1(
        &CONSTRUCTOR.replacen("pub\n    (\n        super\n    )", "pub(crate)", 1)
    ));
    assert!(!exact_test_only_context_constructor_v1(
        &CONSTRUCTOR.replacen(
            "canonical_map_set_digest: [u8; 32]",
            "canonical_map_set_digest: [u16; 32]",
            1
        )
    ));
    assert!(!exact_test_only_context_constructor_v1(
        &CONSTRUCTOR.replacen(
            "Ok(Self { profile_digest: [0; 32] })",
            "Err(ZkAmsMkheErrorV1::InvalidPhase23Fold)",
            1
        )
    ));
    let extend_context_impl =
        |item: &str| CONSTRUCTOR.replacen("\n}\n", &format!("\n{item}\n}}\n"), 1);
    const BENIGN_METHOD: &str = r##"
#[inline]
pub(super) fn inspect<T, F>(&self, value: T, inspect: F) -> u8
where
    T: Copy,
    F: for<'a> Fn(&'a Self, T) -> u8,
{
    const NOTE: &str = "#[cfg(not(test))] fn from_raw() -> Self { Self::new() }";
    fn nested_from_raw<T: Default>() -> T where T: Default { T::default() }
    inspect(self, value)
}
pub(super) const fn version(&self) -> u8 { 1 }
"##;
    assert!(exact_test_only_context_constructor_v1(
        &extend_context_impl(BENIGN_METHOD)
    ));
    const PRODUCTION_FACTORY: &str = r#"
#[cfg(not(test))]
pub /* split */
(
    super
)
fn from_raw<T>(source: T) -> Self
where
    T: Copy,
{
    let _decoy = "fn nested() -> ZkAmsPhase23RnsLinkContextV1";
    Self { profile_digest: [0; 32], ..source }
}
"#;
    assert!(!exact_test_only_context_constructor_v1(
        &extend_context_impl(PRODUCTION_FACTORY)
    ));
    assert!(!exact_test_only_context_constructor_v1(
        &extend_context_impl(
            "#[cfg(test)] pub(super) fn from_raw() -> \
             ZkAmsPhase23RnsLinkContextV1 { loop {} }"
        )
    ));
    assert!(!exact_test_only_context_constructor_v1(
        &extend_context_impl(
            "pub(super) fn publish(out: &mut Option<Self>, source: Self) { \
             *out = Some(Self { ..source }); }"
        )
    ));
    assert!(!exact_test_only_context_constructor_v1(
        &extend_context_impl(
            "#[cfg(not(test))] pub(super) const RAW: Self = \
             Self { profile_digest: [0; 32] };"
        )
    ));
    assert!(!exact_test_only_context_constructor_v1(&format!(
        "{CONSTRUCTOR}\nimpl ZkAmsPhase23RnsLinkContextV1 where Self: Sized {{ \
         pub(super) fn from_raw() -> Self {{ loop {{}} }} }}"
    )));
    let extend_context_module = |item: &str| format!("{CONSTRUCTOR}\n{item}");
    for production_mint in [
        r#"
#[cfg(not(test))]
pub /* split visibility */
(
    super
)
const RAW_CONTEXT: ZkAmsPhase23RnsLinkContextV1 =
    ZkAmsPhase23RnsLinkContextV1 { profile_digest: [0; 32] };
"#,
        r#"
pub(super) static RAW_CONTEXT: Option<ZkAmsPhase23RnsLinkContextV1> = None;
"#,
        r#"
pub(super) fn from_raw() -> ZkAmsPhase23RnsLinkContextV1 {
    ZkAmsPhase23RnsLinkContextV1 { profile_digest: [0; 32] }
}
"#,
        r#"
type RawContext = ZkAmsPhase23RnsLinkContextV1;
type ContextAliasChain = RawContext;
"#,
        r#"
trait ContextFactory { type Output; fn mint() -> Self::Output; }
struct ContextFactoryImpl;
impl ContextFactory for ContextFactoryImpl {
    type Output = ZkAmsPhase23RnsLinkContextV1;
    fn mint() -> Self::Output {
        crate::vega::zk_ams::mkhe::phase23_rns_link::ZkAmsPhase23RnsLinkContextV1 {
            profile_digest: [0; 32],
        }
    }
}
"#,
        r#"
pub(super) fn qualified_literal(source: ZkAmsPhase23RnsLinkContextV1) -> u8 {
    let _mint = super::phase23_rns_link::ZkAmsPhase23RnsLinkContextV1 {
        profile_digest: source.profile_digest,
    };
    0
}
"#,
        r#"
pub(super) fn output_parameter(
    out: &mut Option<ZkAmsPhase23RnsLinkContextV1>,
) {
    let _ = out;
}
"#,
        r#"
pub(super) fn generic_factory<T: ContextFactory<Output = ZkAmsPhase23RnsLinkContextV1>>() {}
"#,
        r#"
pub(super) fn where_factory<T>()
where
    T: ContextFactory<Output = ZkAmsPhase23RnsLinkContextV1>,
{}
"#,
        r#"
pub(super) fn opaque_factory()
    -> impl ContextFactory<Output = ZkAmsPhase23RnsLinkContextV1>
{
    loop {}
}
"#,
        r#"
pub(super) fn invoke_local_factory() -> u8 {
    fn local() -> ZkAmsPhase23RnsLinkContextV1 {
        ZkAmsPhase23RnsLinkContextV1 { profile_digest: [0; 32] }
    }
    let _mint = local();
    0
}
"#,
        r#"
pub(super) fn nested_const_factory() -> u8 {
    const LOCAL: ZkAmsPhase23RnsLinkContextV1 =
        ZkAmsPhase23RnsLinkContextV1 { profile_digest: [0; 32] };
    let _ = LOCAL;
    0
}
"#,
        r#"
pub(super) fn nested_static_storage() -> u8 {
    static LOCAL: Option<ZkAmsPhase23RnsLinkContextV1> = None;
    0
}
"#,
        r#"
pub(super) fn nested_alias_factory() -> u8 {
    type LocalContext = ZkAmsPhase23RnsLinkContextV1;
    fn local() -> LocalContext { loop {} }
    let _mint = local();
    0
}
"#,
        r#"
pub(super) fn nested_impl_factory() -> u8 {
    struct LocalFactory;
    impl LocalFactory {
        fn local() -> ZkAmsPhase23RnsLinkContextV1 {
            ZkAmsPhase23RnsLinkContextV1 { profile_digest: [0; 32] }
        }
    }
    let _mint = LocalFactory::local();
    0
}
"#,
        r#"
pub(super) fn nested_trait_factory() -> u8 {
    trait LocalFactory {
        fn local() -> ZkAmsPhase23RnsLinkContextV1;
    }
    0
}
"#,
        r#"
pub(super) fn cfg_not_test_local_factory() -> u8 {
    #[cfg(not(test))]
    fn local() -> ZkAmsPhase23RnsLinkContextV1 {
        ZkAmsPhase23RnsLinkContextV1 { profile_digest: [0; 32] }
    }
    let _mint = local();
    0
}
"#,
        r#"
pub(super) fn nested_closure_factory() -> u8 {
    let local = || -> ZkAmsPhase23RnsLinkContextV1 {
        ZkAmsPhase23RnsLinkContextV1 { profile_digest: [0; 32] }
    };
    let _mint = local();
    0
}
"#,
        r#"
macro_rules! mint_raw_context {
    () => {
        ZkAmsPhase23RnsLinkContextV1 { profile_digest: [0; 32] }
    };
}
"#,
        r#"
pub(super) fn nested_macro_factory() -> u8 {
    macro_rules! local {
        () => {
            ZkAmsPhase23RnsLinkContextV1 { profile_digest: [0; 32] }
        };
    }
    let _mint = local!();
    0
}
"#,
    ] {
        assert!(!exact_test_only_context_constructor_v1(
            &extend_context_module(production_mint)
        ));
    }
    const BENIGN_MODULE_ITEMS: &str = r#"
type UnrelatedAlias = [u8; 32];
const UNRELATED_CONST: UnrelatedAlias = [0; 32];
static UNRELATED_STATIC: Option<UnrelatedAlias> = None;
trait UnrelatedFactory { type Output; fn mint() -> Self::Output; }
struct UnrelatedFactoryImpl;
impl UnrelatedFactory for UnrelatedFactoryImpl {
    type Output = UnrelatedAlias;
    fn mint() -> Self::Output { [0; 32] }
}
struct ContextReader;
impl ContextReader {
    fn consume<F>(
        context: ZkAmsPhase23RnsLinkContextV1,
        inspect: F,
    ) -> u8
    where
        F: for<'a> Fn(&'a ZkAmsPhase23RnsLinkContextV1) -> u8,
    {
        fn nested_read_only(context: &ZkAmsPhase23RnsLinkContextV1) -> u8 {
            context.profile_digest[0]
        }
        struct NestedReader;
        impl NestedReader {
            fn inspect(context: &ZkAmsPhase23RnsLinkContextV1) -> u8 {
                context.profile_digest[0]
            }
        }
        type LocalByte = u8;
        const LOCAL_VERSION: LocalByte = 1;
        static LOCAL_ENABLED: bool = true;
        macro_rules! identity {
            ($value:expr) => { $value };
        }
        #[cfg(test)]
        fn nested_test_only_factory() -> ZkAmsPhase23RnsLinkContextV1 {
            ZkAmsPhase23RnsLinkContextV1 { profile_digest: [0; 32] }
        }
        let unrelated_closure = |value: u8| value;
        inspect(&context)
            ^ nested_read_only(&context)
            ^ NestedReader::inspect(&context)
            ^ unrelated_closure(identity!(LOCAL_VERSION))
            ^ u8::from(LOCAL_ENABLED)
    }
}
#[cfg(test)]
type TestContextSignature = fn(ZkAmsPhase23RnsLinkContextV1) -> [u8; 32];
#[cfg(test)]
const TEST_CONTEXT_SIGNATURE: TestContextSignature =
    |context| context.profile_digest;
"#;
    assert!(exact_test_only_context_constructor_v1(
        &extend_context_module(BENIGN_MODULE_ITEMS)
    ));

    const SEAL: &str = r#"
enum Phase23ContextCorrespondenceSealV1 {
    #[cfg(test)]
    TestOnly,
}
#[allow(dead_code)]
fn materialize_encrypt_and_publish_phase23_source_v1<I, R, K, P>(
    _correspondence: Phase23ContextCorrespondenceSealV1,
    _marker: PhantomData<(I, R, K, P)>,
) {}
"#;
    assert!(exact_test_only_correspondence_seal_v1(&format!(
        "{DECOYS}{SEAL}"
    )));
    assert!(!exact_test_only_correspondence_seal_v1(&SEAL.replacen(
        "#[cfg(test)]",
        "#[cfg(any(test, feature = \"escape\"))]",
        1
    )));
    assert!(!exact_test_only_correspondence_seal_v1(&SEAL.replacen(
        "fn materialize_encrypt_and_publish_phase23_source_v1",
        "pub(crate) fn materialize_encrypt_and_publish_phase23_source_v1",
        1
    )));
    assert!(!exact_test_only_correspondence_seal_v1(&format!(
        "{SEAL}\nimpl Phase23ContextCorrespondenceSealV1 {{ fn mint() {{}} }}"
    )));
    assert!(contains_no_words_v1(
        DECOYS,
        &[
            "Phase23ContextCorrespondenceSealV1",
            "materialize_encrypt_and_publish_phase23_source_v1"
        ]
    ));

    const SELF_SOURCE: &str = include_str!("incremental_source_phase23_tests.rs");
    assert!(SELF_SOURCE.len() <= RUST_ORACLE_MAX_BYTES_V1);
    assert!(!exact_private_path_module_v1(
        SELF_SOURCE,
        "incremental_source_phase23",
        "incremental_source_phase23.rs"
    ));
    assert!(!exact_test_only_context_constructor_v1(SELF_SOURCE));
    assert!(!whole_module_context_mint_inventory_v1(SELF_SOURCE));
    assert!(!exact_test_only_correspondence_seal_v1(SELF_SOURCE));
    for malformed in ["/*", "\"", "{]", "r#\"unterminated"] {
        assert!(rust_tokens_v1(malformed).is_none());
    }
    assert!(rust_tokens_v1(&" ".repeat(RUST_ORACLE_MAX_BYTES_V1 + 1)).is_none());
}
#[test]
fn source_files_remain_below_the_global_budget_without_exceptions() {
    assert!(
        include_str!("incremental_source_phase23.rs")
            .lines()
            .count()
            <= 900
    );
    assert!(
        include_str!("incremental_source_phase23_tests.rs")
            .lines()
            .count()
            <= 2_850
    );
}
